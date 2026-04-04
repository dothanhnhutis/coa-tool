use anyhow::Result as AnyResult;
use coa_filter_v1::{Args, CsvColumns, Reports, args_parse, read_csv, search};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::{collections::HashMap, fs, path::Path, process};

// cli: cargo run -- -l coa_filter.csv
fn main() -> AnyResult<()> {
    // Header
    println!("{}", "═".repeat(60).cyan());
    println!("{}", "  🔍  COA PDF Filter  v1.0.0".cyan().bold());
    println!("{}", "═".repeat(60).cyan());

    let args: Args = args_parse()?;

    // Tạo thư mục đích
    if let Some(output_dir) = &args.output_dir {
        fs::create_dir_all(&output_dir)?
        // .with_context(|| format!("Không tạo được thư mục: {}", output_dir.display()))?;
    }

    let csv_col = CsvColumns {
        material_id: args.col_material_id_name,
        batch_no: args.col_batch_no_name,
        expiration_date: args.col_expiration_date_name,
    };

    let entries = read_csv(&args.list_file, &csv_col)?;

    if entries.is_empty() {
        return Ok(());
    }

    match search(entries, &args.output_dir) {
        Ok(reports) => {
            if let Some(output_dir) = &args.output_dir {
                let copy_pb = ProgressBar::new(reports.len() as u64);
                copy_pb.set_style(
                    ProgressStyle::with_template(
                        "   [{elapsed_precise}] {bar:40.green/blue} {pos}/{len} {msg}",
                    )
                    .unwrap(),
                );

                let mut copied = 0;
                let mut copy_errors = 0;

                // Track tên file trùng
                let mut name_counter: HashMap<String, usize> = HashMap::new();

                for m in &reports {
                    let filename = m
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    // Xử lý tên file trùng
                    let count = name_counter.entry(filename.clone()).or_insert(0);
                    let dest_filename = if *count == 0 {
                        filename.clone()
                    } else {
                        let stem = Path::new(&filename)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let ext = Path::new(&filename)
                            .extension()
                            .map(|e| format!(".{}", e.to_string_lossy()))
                            .unwrap_or_default();
                        format!("{}_({}){}", stem, count, ext)
                    };
                    *count += 1;

                    let dest = output_dir.join(&dest_filename);
                    copy_pb.set_message(dest_filename.clone());

                    match fs::copy(&m.path, &dest) {
                        Ok(_) => {
                            copied += 1;
                            // if args.verbose {
                            //     copy_pb.println(format!("  ✅ Copied: {}", dest_filename));
                            // }
                        }
                        Err(e) => {
                            copy_errors += 1;
                            copy_pb.println(format!(
                                "  {}",
                                format!("❌ Lỗi copy {}: {}", dest_filename, e).red()
                            ));
                        }
                    }
                    copy_pb.inc(1);
                }
                copy_pb.finish_with_message("Hoàn tất!");
                println!();
            }
        }
        Err(e) => {
            eprintln!("Lỗi: {e:#?}");
            process::exit(1);
        }
    }

    Ok(())
}
