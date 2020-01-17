use chrono::NaiveDate;
use decimal::d128;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::ops::Neg;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "csv2beancount",
    about = "convert transactions in CSV to beancount format"
)]
struct Opt {
    #[structopt(short = "c")]
    csv_path: PathBuf,
    #[structopt(short = "y")]
    yaml_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct YamlConfig {
    csv: Config,
    transactions: Option<HashMap<String, TransactionRule>>,
}

#[derive(Debug, Deserialize)]
struct Config {
    currency: String,
    processing_account: String,
    default_account: String,
    date_format: String,
    date: i64,
    amount_in: i64,
    amount_out: i64,
    description: i64,
    /// The payee of the transaction. Will be omitted if empty.
    payee: Option<i64>,
    delimiter: Option<char>,
    skip: Option<i64>,
    toggle_sign: Option<bool>,
    quote: Option<char>,
}

#[derive(Debug, Deserialize)]
struct TransactionRule {
    account: Option<String>,
    info: Option<String>,
}

impl TransactionRule {
    fn info(&self) -> Option<&str> {
        self.info.as_ref().map(|s| s.as_str())
    }

    fn account(&self) -> Option<&str> {
        self.account.as_ref().map(|s| s.as_str())
    }
}

#[derive(Debug)]
struct Transaction<'a> {
    date: NaiveDate,
    processing_account: &'a str,
    other_account: &'a str,
    currency: &'a str,
    magnitude: d128,
    payee: Option<&'a str>,
    description: &'a str,
}

impl<'a> std::fmt::Display for Transaction<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            fmt,
            r#"{} * {} "{}"
  {} {} {}
  {} {} {}"#,
            self.date,
            if let Some(payee) = self.payee {
                format!(r#""{}""#, payee)
            } else {
                "".into()
            },
            self.description,
            self.processing_account,
            self.magnitude,
            self.currency,
            self.other_account,
            self.magnitude.neg(),
            self.currency
        )
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Opt::from_args();

    let yaml_file = std::fs::File::open(&opt.yaml_path)?;
    let root_config: YamlConfig = serde_yaml::from_reader(yaml_file)?;
    let config = root_config.csv;
    let transaction_rules = root_config.transactions;
    let csv_file = std::fs::File::open(opt.csv_path)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(config.delimiter.map(|del| del as u8).unwrap_or(b','))
        .quote(config.quote.map(|del| del as u8).unwrap_or(b'\"'))
        .has_headers(false)
        .from_reader(csv_file);

    let mut first = true;
    for result in rdr.records().skip(config.skip.unwrap_or(0) as usize) {
        let record = result?;

        if first {
            first = false;
        } else {
            println!();
        }

        let payee = config
            .payee
            .map(|payee| &record[payee as usize])
            .filter(|payee| !payee.is_empty());
        let description = &record[config.description as usize];
        let date = NaiveDate::parse_from_str(&record[config.date as usize], &config.date_format)?;

        // The current applicable rule, if any.
        let current_transaction_rule = transaction_rules
            .as_ref()
            .and_then(|rules| rules.get(description));

        let t = Transaction {
            date,
            description: current_transaction_rule
                .and_then(TransactionRule::info)
                .unwrap_or(description),
            payee,
            processing_account: &config.processing_account,
            other_account: current_transaction_rule
                .and_then(TransactionRule::account)
                .unwrap_or(&config.default_account),
            magnitude: {
                let in_amount = &record[config.amount_in as usize];
                let out_amount = &record[config.amount_out as usize];
                let amt = if let Ok(amt) = in_amount.parse::<d128>() {
                    amt
                } else if let Ok(amt) = out_amount.parse::<d128>() {
                    amt.neg()
                } else {
                    Err(format!(
                        "Could not parse either in or out amounts for {}",
                        description
                    ))?
                };
                if config.toggle_sign == Some(true) {
                    amt.neg()
                } else {
                    amt
                }
            },
            currency: &config.currency,
        };

        println!("{}", t)
    }

    Ok(())
}
