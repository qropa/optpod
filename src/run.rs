use core::fmt;
use std::{cmp::Reverse, collections::BinaryHeap};

use crate::settings;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use rayon::prelude::*;
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

pub fn run(args: RunArgs) -> Result<()> {
    let seed = args.seed;
    let start = seed.0;
    let end = seed.1;

    let _config = settings::read_settings()?;

    let pool = rayon::ThreadPoolBuilder::new().num_threads(24).build()?;

    let next_seed = Arc::new(Mutex::new(start));
    let res_que = Arc::new(Mutex::new(BinaryHeap::new()));
    let next_print_seed = Arc::new(Mutex::new(start));

    pool.install(|| {
        (start..=end).into_par_iter().for_each(|_| {
            let seed = {
                let mut next_seed = next_seed.lock().unwrap();
                let seed = *next_seed as usize;
                *next_seed += 1;
                seed
            };
            let res = single_exec(seed, "command").unwrap();
            let mut res_que = res_que.lock().unwrap();
            let mut next_print_seed = next_print_seed.lock().unwrap();
            res_que.push(Reverse(res));
            while res_que.len() > 0 && res_que.peek().unwrap().0.seed == *next_print_seed as usize {
                let res = res_que.pop().unwrap().0;
                println!("{}", res);
                *next_print_seed += 1;
            }
        });
    });

    Ok(())
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Ord)]
struct ExecResult {
    seed: usize,
    score: String,
    relative: Option<String>,
    data: Vec<(String, String)>,
}
impl fmt::Display for ExecResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {:>09}{} {}",
            format!("{:>04}", self.seed).green(),
            self.score,
            if let Some(relative) = self.relative.as_ref() {
                let relative = relative.parse::<f64>().unwrap();
                if relative < 50. {
                    format!(" {:>6.2}", relative).red()
                } else if relative < 80. {
                    format!(" {:>6.2}", relative).yellow()
                } else if relative < 95. {
                    format!(" {:>6.2}", relative).green()
                } else {
                    format!(" {:>6.2}", relative).blue()
                }
            } else {
                "".to_string().white()
            },
            self.data
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" ")
        )
    }
}

fn single_exec(seed: usize, command: &str) -> Result<ExecResult> {
    let start_time = get_time();
    let end_time = start_time + rnd::nextf() * 2.0;
    while get_time() < end_time {}
    let res = ExecResult {
        seed,
        score: rnd::get(1000000).to_string(),
        relative: Some((rnd::nextf() * 100.0).to_string()),
        data: vec![
            ("a".to_string(), rnd::next().to_string()),
            ("b".to_string(), "200".to_string()),
            ("sum".to_string(), "16.5".to_string()),
        ],
    };
    Ok(res)
}

fn sum(a: usize, b: usize) -> usize {
    let start_time = get_time();
    let end_time = start_time + rnd::nextf() * 5.0;
    while get_time() < end_time {}
    a + b
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
