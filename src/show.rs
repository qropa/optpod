use crate::run;
use crate::settings;
use anyhow::Result;
use clap::Parser;
use std::io::BufRead;

pub fn show(args: ShowArgs) -> Result<()> {
    let config = settings::read_settings()?;

    let id = args.id.unwrap_or(config.default_id.clone());

    let result_file_path = format!("{}/{}.jsonl", config.result_dir, id);
    let result_file = std::fs::File::open(&result_file_path)?;
    let result = std::io::BufReader::new(result_file)
        .lines()
        .map(|line| serde_json::from_str::<run::ExecResult>(&line.unwrap()).unwrap())
        .collect::<Vec<_>>();

    let variable = args.valuable;
    let start = args.start;
    let end = args.end;
    let step = args.step;

    let mut ranges = vec![];
    let mut current = start;
    while current < end {
        ranges.push((current, current + step));
        current += step;
    }
    let mut buckets = vec![(0.0, 0.0, 0); ranges.len()];

    for res in result {
        if let Some((_, value)) = res.data.iter().find(|(key, _)| key == &variable) {
            let value = value.parse::<f64>().unwrap();
            if let Some(idx) = ranges
                .iter()
                .position(|(start, end)| start <= &value && &value < end)
            {
                buckets[idx].0 += res.score;
                buckets[idx].1 += res.relative;
                buckets[idx].2 += 1;
            }
        }
    }

    println!(
        "+--------------------------------+--------------------+--------------------+----------+"
    );
    println!(
        "| Range {:<24} | Avg Score          | Avg Relative       | Count    |",
        variable
    );
    println!(
        "+--------------------------------+--------------------+--------------------+----------+"
    );

    for i in 0..buckets.len() {
        let range = ranges[i];
        let (sum_score, sum_relative, count) = buckets[i];
        let avg_score = if count > 0 {
            sum_score / count as f64
        } else {
            0.0
        };
        let avg_relative = if count > 0 {
            sum_relative / count as f64
        } else {
            0.0
        };
        println!(
            "| {:<30} | {:<18.4} | {:<18.4} | {:<8} |",
            format!("{:.1} - {:.1}", range.0, range.1),
            avg_score,
            avg_relative,
            count
        );
    }

    println!(
        "+--------------------------------+--------------------+--------------------+----------+"
    );

    Ok(())
}

#[derive(Parser)]
pub struct ShowArgs {
    valuable: String,
    start: f64,
    end: f64,
    step: f64,
    id: Option<String>,
}
