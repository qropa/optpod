use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use toml;

pub const SETTING_FILE: &str = "optpod_config.toml";

pub fn init() -> Result<()> {
    if std::path::Path::new(SETTING_FILE).exists() {
        println!("Config file already exists. Do you want to overwrite it? [y/N]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() != "y" {
            println!("Aborted");
            return Ok(());
        }
    }
    let config = Config {
        in_dir: "tools/in".to_string(),
        result_dir: ".".to_string(),
        tests_dir: "tests".to_string(),
        standard_output_extension: "out".to_string(),
        standard_error_extension: "err".to_string(),
        cmd_tester: "./target/release/a".to_string(),
        extraction_regex: r"^\s*\[DATA\]\s+(?P<VARIABLE>[a-zA-Z]\w*)\s*=\s*(?P<VALUE>\S+)\s*$"
            .to_string(),
        scoring: "min".to_string(),
        threads_no: 0,
        relative_score: false,
        auto_save_best: false,
    };
    let toml = toml::to_string_pretty(&config)?;

    let mut file = std::fs::File::create(SETTING_FILE)?;
    file.write_all(toml.as_bytes())?;
    println!("Config file created");

    Ok(())
}

pub fn read_settings() -> Result<Config> {
    if !std::path::Path::new(SETTING_FILE).exists() {
        anyhow::bail!("Config file not found. Run `optpod init` first");
    }
    let config = toml::from_str::<Config>(&std::fs::read_to_string(SETTING_FILE)?)?;
    Ok(config)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub in_dir: String,
    pub result_dir: String,
    pub tests_dir: String,
    pub standard_output_extension: String,
    pub standard_error_extension: String,
    pub cmd_tester: String,
    pub extraction_regex: String,
    pub scoring: String,
    pub threads_no: u32,
    pub relative_score: bool,
    pub auto_save_best: bool,
}
