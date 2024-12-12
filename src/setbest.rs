use crate::run;
use crate::settings;
use anyhow::Result;
use clap::Parser;
use std::io::{BufRead, Write};

#[derive(Parser)]
pub struct SetBestArgs {
    id: Option<String>,
}

pub fn set_best(args: SetBestArgs) -> Result<()> {
    let config = settings::read_settings()?;

    let id = args.id.unwrap_or(config.default_dir.clone());

    let result_file_path = format!("{}/{}.jsonl", config.result_dir, id);
    let best_file_path = format!("{}/best.jsonl", config.result_dir);
    let prev_best_file_path = format!("{}/prev_best.jsonl", config.result_dir);
    if !std::path::Path::new(&config.result_dir).exists() {
        std::fs::create_dir(&config.result_dir)?;
    }
    for path in [&result_file_path, &best_file_path, &prev_best_file_path].iter() {
        if !std::path::Path::new(path).exists() {
            std::fs::File::create(path)?;
        }
    }
    std::fs::copy(&best_file_path, &prev_best_file_path)?;
    let result_file = std::fs::File::open(&result_file_path)?;
    let results = std::io::BufReader::new(result_file)
        .lines()
        .map(|line| serde_json::from_str::<run::ExecResult>(&line.unwrap()).unwrap())
        .collect::<Vec<_>>();
    let best_file = std::fs::File::create(&best_file_path)?;

    // 空なら空のVecを作成
    let mut bests = if best_file.metadata()?.len() == 0 {
        Vec::new()
    } else {
        std::io::BufReader::new(best_file)
            .lines()
            .map(|line| serde_json::from_str::<run::ExecResult>(&line.unwrap()).unwrap())
            .collect::<Vec<_>>()
    };

    for result in results {
        if let Some(best) = bests.iter_mut().find(|best| best.seed == result.seed) {
            if best.score < result.score {
                *best = result;
            }
        } else {
            bests.push(result);
        }
    }
    bests.sort_by_key(|best| best.seed);
    let best_file = std::fs::File::create(&best_file_path)?;
    let mut best_file = std::io::BufWriter::new(best_file);
    for best in bests {
        writeln!(best_file, "{}", serde_json::to_string(&best)?)?;
    }
    eprintln!("Best updated");
    Ok(())
}
