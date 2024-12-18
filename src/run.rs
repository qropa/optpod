use core::f64;
use std::collections::HashMap;
use std::f64::NAN;
use std::{cmp::Reverse, collections::BinaryHeap};

use crate::settings::{self, Config};

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use num_cpus;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

#[derive(Parser)]
pub struct RunArgs {
    #[arg(value_parser = parse_seed)]
    seed: (u32, u32),
    id: Option<String>,
}

fn parse_seed(s: &str) -> Result<(u32, u32)> {
    let mut v = vec![];
    for p in s.split('-') {
        let value = p.parse()?;
        v.push(value);
    }
    let seed = match v.len() {
        1 => (0, v[0]),
        2 => (v[0], v[1]),
        _ => anyhow::bail!("Invalid seed format. Must be 'value' or 'value-value'"),
    };
    if seed.0 > seed.1 {
        anyhow::bail!(
            "Invalid seed range. First value must be less than or equal to the second value"
        );
    }
    Ok(seed)
}
pub fn run(args: RunArgs) -> Result<(), anyhow::Error> {
    let seed = args.seed;
    let start = seed.0;
    let end = seed.1;

    let config = settings::read_settings()?;
    let num_threads = if config.threads_no > 0 {
        config.threads_no as usize
    } else {
        num_cpus::get()
    };
    if !std::path::Path::new(&config.result_dir).exists() {
        std::fs::create_dir(&config.result_dir)?;
    }
    let id = args.id.unwrap_or(config.default_id.clone());
    if &id == "best" || &id == "prev_best" {
        anyhow::bail!("Invalid id: {}. Must not be 'best' or 'prev_best'", id);
    }
    let sample_output_file_path = replace_placeholder2(&config.output, 0, &id);
    let output_dir = std::path::Path::new(&sample_output_file_path)
        .parent()
        .unwrap();
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
    }
    let sample_error_file_path = replace_placeholder2(&config.error, 0, &id);
    let error_dir = std::path::Path::new(&sample_error_file_path)
        .parent()
        .unwrap();
    if !error_dir.exists() {
        std::fs::create_dir_all(error_dir)?;
    }

    let res_file_path = format!("{}/{}.jsonl", config.result_dir, id);
    let res_file = Arc::new(Mutex::new(std::fs::File::create(&res_file_path)?));
    let best_file_path = format!("{}/best.jsonl", config.result_dir);
    if !std::path::Path::new(&best_file_path).exists() {
        std::fs::File::create(&best_file_path)?;
    }
    let best_file = std::fs::File::open(&best_file_path)?;
    let mut best_scores = HashMap::new();
    let reader = BufReader::new(best_file);
    for line in reader.lines() {
        let line = line?;
        let res: ExecResult = serde_json::from_str(&line)?;
        best_scores.insert(res.seed, res.score);
    }

    let next_seed = Arc::new(Mutex::new(start));
    let res_que = Arc::new(Mutex::new(BinaryHeap::new()));
    let next_print_seed = Arc::new(Mutex::new(start));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()?;

    let sum_score = Arc::new(Mutex::new(0.0));
    let sum_log_score = Arc::new(Mutex::new(0.0));
    let sum_relative = Arc::new(Mutex::new(0.0));
    pool.install(|| {
        (start..=end).into_par_iter().for_each(|_| {
            let seed = {
                let mut next_seed = next_seed.lock().unwrap();
                let seed = *next_seed as usize;
                *next_seed += 1;
                seed
            };
            let res = single_exec(seed, &id, &config, &best_scores).unwrap();

            *sum_score.lock().unwrap() += res.score;
            *sum_log_score.lock().unwrap() += res.score.ln();
            *sum_relative.lock().unwrap() += res.relative;

            let mut res_que = res_que.lock().unwrap();
            let mut next_print_seed = next_print_seed.lock().unwrap();
            let mut res_file = res_file.lock().unwrap();
            res_que.push(Reverse(res));
            while res_que.len() > 0 && res_que.peek().unwrap().0.seed == *next_print_seed as usize {
                let res = res_que.pop().unwrap().0;
                println!("{}", res);
                *next_print_seed += 1;
                writeln!(res_file, "{}", serde_json::to_string(&res).unwrap()).unwrap();
            }
        });
    });

    let mean_score = *sum_score.lock().unwrap() / (end - start + 1) as f64;
    let mean_log_score = *sum_log_score.lock().unwrap() / (end - start + 1) as f64;
    let mean_relative = *sum_relative.lock().unwrap() / (end - start + 1) as f64;
    println!("Avg Score: {}", mean_score);
    println!("Avg Log Score: {}", mean_log_score);
    println!("Avg relative: {}", mean_relative);

    Ok(())
}

#[derive(Debug, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct ExecResult {
    pub seed: usize,
    pub score: f64,
    pub relative: f64,
    pub data: Vec<(String, String)>,
}
impl Ord for ExecResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.partial_cmp(&other.score).unwrap()
    }
}
impl Eq for ExecResult {
    fn assert_receiver_is_total_eq(&self) {}
}

impl core::fmt::Display for ExecResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{}: {:>9}{} {}",
            format!("{:>04}", self.seed).green(),
            self.score,
            if self.relative < 50. {
                format!(" {:>6.2}", self.relative).red()
            } else if self.relative < 80. {
                format!(" {:>6.2}", self.relative).yellow()
            } else if self.relative < 95. {
                format!(" {:>6.2}", self.relative).green()
            } else {
                format!(" {:>6.2}", self.relative).blue()
            },
            self.data
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn single_exec(
    seed: usize,
    id: &str,
    config: &Config,
    best_scores: &HashMap<usize, f64>,
) -> Result<ExecResult> {
    let input_file_path = replace_placeholder(&config.input, seed);
    let input_file = std::fs::File::open(&input_file_path)?;
    let output = std::process::Command::new(&config.cmd_tester)
        .stdin(std::process::Stdio::from(input_file))
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to execute seed:{}, Error:{:?}",
            seed,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut res = ExecResult {
        seed,
        score: NAN,
        relative: 0.0,
        data: vec![],
    };
    for line in String::from_utf8_lossy(&output.stderr).lines() {
        if let Some(caps) = regex::Regex::new(&config.extraction_regex)?.captures(line) {
            if &caps["VARIABLE"] == "score" {
                res.score = caps["VALUE"].parse()?;
            } else {
                if let Some((_, v)) = res.data.iter_mut().find(|(k, _)| *k == caps["VARIABLE"]) {
                    *v = caps["VALUE"].to_string();
                } else {
                    res.data
                        .push((caps["VARIABLE"].to_string(), caps["VALUE"].to_string()));
                }
            }
        }
    }
    let best_score = best_scores.get(&seed).copied().unwrap_or(f64::NAN);
    if best_score.is_nan() || res.score.is_nan() {
        res.relative = 0.0;
    } else {
        match config.scoring.as_str() {
            "min" => {
                res.relative = best_score / res.score * 100.;
            }
            "max" => {
                res.relative = res.score / best_score * 100.;
            }
            _ => {
                anyhow::bail!("Invalid scoring method: {}", config.scoring);
            }
        }
    }
    let output_file_path = replace_placeholder2(&config.output, seed, id);
    eprintln!("output_file_path: {}", output_file_path);
    let mut output_file = std::fs::File::create(&output_file_path)?;
    eprintln!("out file exists");
    output_file.write_all(&output.stdout)?;
    let error_file_path = replace_placeholder2(&config.error, seed, id);
    let mut error_file = std::fs::File::create(&error_file_path)?;
    error_file.write_all(&output.stderr)?;
    Ok(res)
}

fn replace_placeholder(s: &str, seed: usize) -> String {
    s.replace("{SEED04}", &format!("{:04}", seed))
        .replace("{SEED}", &format!("{}", seed))
}

fn replace_placeholder2(s: &str, seed: usize, id: &str) -> String {
    s.replace("{SEED04}", &format!("{:04}", seed))
        .replace("{SEED}", &format!("{}", seed))
        .replace("{ID}", &format!("{}", id))
}
