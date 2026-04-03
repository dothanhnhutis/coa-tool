use anyhow::Result as AnyResult;
use coa_filter_v1::{Args, CsvColumns, args_parse, read_csv, search};
use colored::Colorize;
use std::process;

// cli: cargo run -- -l coa_filter.csv
fn main() -> AnyResult<()> {
    // Header
    println!("{}", "═".repeat(60).cyan());
    println!("{}", "  🔍  COA PDF Filter  v1.0.0".cyan().bold());
    println!("{}", "═".repeat(60).cyan());

    let args: Args = args_parse()?;

    // Tạo thư mục đích
    // if !args.dry_run {
    //     fs::create_dir_all(&args.output_dir)
    //         .with_context(|| format!("Không tạo được thư mục: {}", args.output_dir.display()))?;
    // }

    let csv_col = CsvColumns {
        material_id: args.col_material_id_name,
        batch_no: args.col_batch_no_name,
        expiration_date: args.col_expiration_date_name,
    };

    let entries = read_csv(&args.list_file, &csv_col)?;

    if entries.is_empty() {
        return Ok(());
    }

    if let Err(e) = search(entries) {
        eprintln!("Lỗi: {e:#?}");
        process::exit(1);
    }

    Ok(())
}
