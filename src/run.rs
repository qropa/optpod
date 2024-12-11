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
use std::sync::{Arc, Mutex};

#[derive(Parser)]
pub struct RunArgs {
    #[arg(short, long, value_parser = parse_seed)]
    seed: (u32, u32),
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
    if !std::path::Path::new(&config.tests_dir).exists() {
        std::fs::create_dir(&config.tests_dir)?;
    }
    if !std::path::Path::new(&config.result_dir).exists() {
        std::fs::create_dir(&config.result_dir)?;
    }
    let res_file_path = format!("{}/result.jsonl", config.result_dir);
    let res_file = Arc::new(Mutex::new(std::fs::File::create(&res_file_path)?));
    let best_file_path = format!("{}/best.jsonl", config.result_dir);
    let best_file = std::fs::File::create(&best_file_path);

    let next_seed = Arc::new(Mutex::new(start));
    let res_que = Arc::new(Mutex::new(BinaryHeap::new()));
    let next_print_seed = Arc::new(Mutex::new(start));

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()?;
    pool.install(|| {
        (start..=end).into_par_iter().for_each(|_| {
            let seed = {
                let mut next_seed = next_seed.lock().unwrap();
                let seed = *next_seed as usize;
                *next_seed += 1;
                seed
            };
            let res = single_exec(seed, &config).unwrap();
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

    Ok(())
}

#[derive(Debug, PartialOrd, PartialEq, Serialize, Deserialize)]
struct ExecResult {
    seed: usize,
    score: f64,
    relative: f64,
    data: Vec<(String, String)>,
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

fn single_exec(seed: usize, config: &Config) -> Result<ExecResult> {
    let input_file_path = format!("tools/in/{:04}.txt", seed);
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
        relative: rnd::nextf() * 100.,
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
    let output_file_path = format!(
        "{}/{:>04}.{}",
        config.tests_dir, seed, config.standard_output_extension
    );
    let mut output_file = std::fs::File::create(&output_file_path)?;
    output_file.write_all(&output.stdout)?;
    let error_file_path = format!(
        "{}/{:>04}.{}",
        config.tests_dir, seed, config.standard_error_extension
    );
    let mut error_file = std::fs::File::create(&error_file_path)?;
    error_file.write_all(&output.stderr)?;
    Ok(res)
}

pub fn get_time() -> f64 {
    static mut START: f64 = -1.0;
    let end = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    unsafe {
        if START < 0.0 {
            START = end;
        }
        end - START
    }
}

pub mod rnd {
    #![allow(dead_code)]
    static mut A: u64 = 1;

    pub fn next() -> u32 {
        unsafe {
            let mut x = A;
            A *= 0xcafef00dd15ea5e5;
            x ^= x >> 22;
            (x >> 22 + (x >> 61)) as u32
        }
    }

    pub fn next64() -> u64 {
        (next() as u64) << 32 | next() as u64
    }

    pub fn nextf() -> f64 {
        unsafe { std::mem::transmute::<u64, f64>(0x3ff0000000000000 | (next() as u64) << 20) - 1. }
    }

    pub fn get(n: usize) -> usize {
        assert!(n <= u32::MAX as usize);
        next() as usize * n >> 32
    }

    pub fn range(a: usize, b: usize) -> usize {
        assert!(a < b);
        get(b - a) + a
    }

    pub fn range_skip(a: usize, b: usize, skip: usize) -> usize {
        assert!(a <= skip && skip < b);
        let n = range(a, b - 1);
        n + (skip <= n) as usize
    }

    pub fn rangei(a: i64, b: i64) -> i64 {
        assert!(a < b);
        get((b - a) as usize) as i64 + a
    }

    pub fn shuffle<T>(list: &mut [T]) {
        for i in (0..list.len()).rev() {
            list.swap(i, get(i + 1));
        }
    }

    pub fn shuffle_iter<T: Copy>(list: &mut [T]) -> impl Iterator<Item = T> + '_ {
        (0..list.len()).rev().map(|i| {
            list.swap(i, get(i + 1));
            list[i]
        })
    }

    // 平均 mu, 標準偏差 sigma の正規分布に従う乱数を生成する
    pub fn sample(mu: f64, sigma: f64) -> f64 {
        let u1 = nextf();
        let u2 = nextf();
        mu + (-2.0 * u1.ln() * sigma * sigma).sqrt() * (2.0 * 3.14159265 * u2).cos()
    }
}
