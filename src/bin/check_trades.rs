//! 批量检查目录下所有 .trade 文件的语法和语义
//!
//! 用法：
//!   cargo run -p trade-meta-compiler --bin check_trades -- ./trades
//!   cargo run -p trade-meta-compiler --bin check_trades -- ./trades ./other_dir

use std::process;

use trade_meta_compiler::{Checker, StrategyParser, builtin_symbol_registry};

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let dirs = if dirs.is_empty() {
        vec![".".to_string()]
    } else {
        dirs
    };

    let parser = StrategyParser::new();
    let registry = builtin_symbol_registry();

    let mut total = 0usize;
    let mut failed = 0usize;

    for dir in &dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("❌ 无法读取目录 {}: {}", dir, e);
                failed += 1;
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("trade") {
                continue;
            }
            total += 1;
            let display = path.display().to_string();

            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ {}: 读取失败: {}", display, e);
                    failed += 1;
                    continue;
                }
            };

            let ast = match parser.parse(&source) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("❌ {}: 语法错误: {}", display, e);
                    failed += 1;
                    continue;
                }
            };

            let mut checker = Checker::new(registry.clone());
            match checker.check(&ast) {
                Ok(_) => {
                    println!("✅ {}", display);
                }
                Err(e) => {
                    eprintln!("❌ {}: 语义错误: {:?}", display, e);
                    failed += 1;
                }
            }
        }
    }

    println!("\n共 {} 个文件，{} 个通过，{} 个失败", total, total - failed, failed);

    if failed > 0 {
        process::exit(1);
    }
}
