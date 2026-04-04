use anyhow::{Context, Result as AnyResult};
use clap::Parser;
use colored::Colorize;
use serde::Deserialize;
use serde_json;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

pub const COA_FILE_BASE_PATH: &'static str = "./data";
pub const COA_BASE_PATH: &'static str = "./data/data.json";

#[derive(Parser, Debug)]
#[command(
    name = "coa_filter",
    about = "🔍 Lọc và copy file COA PDF theo danh sách (tên, số lô, ngày sản xuất)",
    version = "1.0.0"
)]
pub struct Args {
    /// File danh sách (CSV) chứa tên sản phẩm, số lô, ngày hết hạn
    #[arg(short = 'l', long, help = "File CSV danh sách cần lọc")]
    pub list_file: PathBuf,

    /// Thư mục đích để copy các file phù hợp
    #[arg(short = 'o', long, help = "Thư mục đích (sẽ được tạo nếu chưa có)")]
    pub output_dir: Option<PathBuf>,

    /// Cột tên sản phẩm trong CSV (mặc định: "material_id")
    #[arg(long, default_value = "material_id")]
    pub col_material_id_name: String,

    /// Cột số lô trong CSV (mặc định: "batch_no")
    #[arg(long, default_value = "batch_no")]
    pub col_batch_no_name: String,

    /// Cột ngày sản xuất trong CSV (mặc định: "expiration_date")
    #[arg(long, default_value = "expiration_date")]
    pub col_expiration_date_name: String,
}

pub fn args_parse() -> AnyResult<Args> {
    let args: Args = Args::parse();

    if !args.list_file.exists() {
        anyhow::bail!("File danh sách không tồn tại: {}", args.list_file.display());
    }

    Ok(args)
}

pub struct ListEntry {
    pub material_id: String,
    pub batch_no: String,
    pub expiration_date: String,
}

pub struct CsvColumns {
    pub material_id: String,
    pub batch_no: String,
    pub expiration_date: String,
}

#[derive(Debug, Deserialize)]
struct Coa {
    batch_no: String,
    expiry_date: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct Material {
    material_name: String,
    coa_list: Vec<Coa>,
}

#[derive(Debug)]
pub struct ResultItem {
    pub material_id: String,
    pub batch_no: String,
    pub expiry_date: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct Reports {
    material_id: String,
    material_name: String,
    batch_no: String,
    expiry_date: String,
    path: String,
    exists_file: bool,
    reason: String,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn file_exists(base: &str, relative: &str) -> bool {
    if relative.is_empty() {
        return false;
    }
    let full_path = format!("{}{}", base, relative);
    std::path::Path::new(&full_path).exists()
}

pub fn read_csv(path: &PathBuf, csv_col: &CsvColumns) -> AnyResult<Vec<ListEntry>> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_path(path)
        .with_context(|| format!("Không đọc được file CSV: {}", path.display()))?;

    let headers = rdr.headers()?.clone();

    // Tìm index cột (case-insensitive)
    let find_col = |name: &str| -> Option<usize> {
        headers
            .iter()
            .position(|h| h.to_lowercase() == name.to_lowercase())
    };

    let idx_material_id = find_col(&csv_col.material_id).ok_or_else(|| {
        anyhow::anyhow!(
            "Không tìm thấy cột '{}' trong CSV. Các cột hiện có: {}",
            &csv_col.material_id,
            headers.iter().collect::<Vec<_>>().join(", ")
        )
    })?;

    let idx_batch_no = find_col(&csv_col.batch_no)
        .ok_or_else(|| anyhow::anyhow!("Không tìm thấy cột '{}' trong CSV", &csv_col.batch_no))?;

    let idx_expiration_date = find_col(&csv_col.expiration_date).ok_or_else(|| {
        anyhow::anyhow!(
            "Không tìm thấy cột '{}' trong CSV",
            &csv_col.expiration_date
        )
    })?;

    let mut entries = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let material_id = record.get(idx_material_id).unwrap_or("").trim().to_string();
        let batch_no = record.get(idx_batch_no).unwrap_or("").trim().to_string();
        let expiration_date = record
            .get(idx_expiration_date)
            .unwrap_or("")
            .trim()
            .to_string();

        if material_id.is_empty() {
            continue;
        }

        entries.push(ListEntry {
            material_id,
            batch_no,
            expiration_date,
        });
    }

    println!();
    if !entries.is_empty() {
        println!(
            "   ✅ Đọc được {} dòng trong danh sách\n",
            entries.len().to_string().green().bold()
        );

        // In bảng kết quả
        println!(
            "  {:<20} {:<20} {}",
            "Material ID".bold(),
            "Batch Number".bold(),
            "Expiration Date".bold()
        );
        println!("  {}", "─".repeat(60));

        for entry in &entries {
            println!(
                "  {:<20} {:<20} {}",
                truncate(&entry.material_id, 18).green(),
                truncate(&entry.batch_no, 18).cyan(),
                truncate(
                    if entry.expiration_date == "" {
                        "--"
                    } else {
                        &entry.expiration_date
                    },
                    13
                )
                .yellow()
            );
        }
    } else {
        println!("{}", "⚠️  Danh sách trống, không có gì để lọc.".yellow());
    }

    Ok(entries)
}

pub fn search(
    entries: Vec<ListEntry>,
    output_path: &Option<PathBuf>,
) -> AnyResult<Vec<ResultItem>> {
    let data = fs::read_to_string(COA_BASE_PATH)?;
    let data: HashMap<String, Material> = serde_json::from_str(&data)?;

    let mut reports: Vec<Reports> = Vec::new();
    let mut results: Vec<ResultItem> = Vec::new();

    for entry in entries {
        let Some(material) = data.get(&entry.material_id) else {
            reports.push(Reports {
                material_id: entry.material_id,
                material_name: "(empty)".into(),
                batch_no: "(empty)".into(),
                expiry_date: "(empty)".into(),
                path: "(empty)".into(),
                exists_file: false,
                reason: "Không tìm thấy mã nguyên liệu trong database.".into(),
            });
            continue;
        };

        let coas: Vec<&Coa> = material
            .coa_list
            .iter()
            .filter(|c| c.batch_no == entry.batch_no)
            .collect();

        if coas.is_empty() {
            reports.push(Reports {
                material_id: entry.material_id,
                material_name: material.material_name.clone(),
                batch_no: entry.batch_no,
                expiry_date: "(empty)".into(),
                path: "(empty)".into(),
                exists_file: false,
                reason: "Mã lô không tồn tại.".into(),
            });
            continue;
        }

        // Filter theo expiration_date nếu có
        let filtered_coas: Vec<&Coa> = if entry.expiration_date.is_empty() {
            coas
        } else {
            coas.into_iter()
                .filter(|c| c.expiry_date == entry.expiration_date)
                .collect()
        };

        if filtered_coas.is_empty() {
            reports.push(Reports {
                material_id: entry.material_id,
                material_name: material.material_name.clone(),
                batch_no: entry.batch_no,
                expiry_date: entry.expiration_date,
                path: "(empty)".into(),
                exists_file: false,
                reason: "Không tìm thấy mã lô với HSD tương tự.".into(),
            });
            continue;
        }

        for coa in filtered_coas {
            if reports
                .iter()
                .any(|r| r.batch_no == coa.batch_no && r.expiry_date == coa.expiry_date)
            {
                continue;
            }

            let full_path = format!("{}{}", COA_FILE_BASE_PATH, coa.path);
            let exists = file_exists(COA_FILE_BASE_PATH, &coa.path);

            let reason = if exists {
                "OK"
            } else {
                "Không tìm thấy file COA."
            };

            // push report
            reports.push(Reports {
                material_id: entry.material_id.clone(),
                material_name: material.material_name.clone(),
                batch_no: coa.batch_no.clone(),
                expiry_date: coa.expiry_date.clone(),
                path: full_path.clone(),
                exists_file: exists,
                reason: reason.into(),
            });

            // push result nếu file tồn tại
            if exists {
                results.push(ResultItem {
                    material_id: entry.material_id.clone(),
                    batch_no: coa.batch_no.clone(),
                    expiry_date: coa.expiry_date.clone(),
                    path: PathBuf::from(full_path.clone()),
                });
            }
        }
    }

    println!();
    if !reports.is_empty() {
        // In bảng kết quả
        println!(
            "  {:<20} {:<20} {:<20} {:<20} {:<50} {:15} {} ",
            "Material ID".bold(),
            "Material Name".bold(),
            "Batch Number".bold(),
            "Expiration Date".bold(),
            "Path".bold(),
            "Exists File".bold(),
            "Reason".bold()
        );
        println!("  {}", "─".repeat(190));

        for r in &reports {
            println!(
                "  {:<20} {:<20} {:<20} {:<20} {:<50} {:15} {} ",
                truncate(&r.material_id, 18).green(),
                truncate(&r.material_name, 18).cyan(),
                truncate(&r.batch_no, 18).cyan(),
                truncate(
                    if r.expiry_date == "" {
                        "--"
                    } else {
                        &r.expiry_date
                    },
                    18
                )
                .yellow(),
                truncate(&r.path, 48).cyan(),
                &r.exists_file,
                truncate(&r.reason, 50)
            );
        }
    }

    println!();

    if !results.is_empty() {
        println!(
            "   ✅ Tìm được {} file COA\n",
            results.len().to_string().green().bold()
        );
    }

    if let Some(output_dir) = output_path {
        let report_path = output_dir.join("filter_report.csv");
        save_report(&reports, &report_path)?;
        println!(
            "\n  📄 Báo cáo lưu tại: {}",
            report_path.display().to_string().yellow()
        );
    }

    Ok(results)
}

pub fn save_report(matches: &[Reports], path: &Path) -> AnyResult<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record([
        "Material ID",
        "Material Name",
        "Batch Number",
        "Expiration Date",
        "Path",
        "Exists File",
        "Reason",
    ])?;
    for m in matches {
        wtr.write_record([
            &m.material_id,
            &m.material_name,
            &m.batch_no,
            &m.expiry_date,
            &m.path,
            &m.exists_file.to_string(),
            &m.reason,
        ])?;
    }
    wtr.flush()?;
    Ok(())
}
