use anyhow::{anyhow, Context, Result};
use csv::Writer;
use html2text::from_read;
use serde_json::Value;
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SecCompany {
    pub cik: String,
    pub ticker: String,
    pub title: String,
}

pub fn fetch_company_by_ticker(ticker: &str) -> Result<SecCompany> {
    let ticker_l = ticker.to_ascii_lowercase();
    let url = "https://www.sec.gov/files/company_tickers.json";
    let data: Value = ureq::get(url)
        .set("User-Agent", "quick-search/0.1 contact@example.com")
        .call()
        .context("fetch SEC company ticker map")?
        .into_json()
        .context("parse SEC company ticker map")?;

    let obj = data.as_object().ok_or_else(|| anyhow!("SEC ticker map was not a JSON object"))?;
    for entry in obj.values() {
        let Some(t) = entry.get("ticker").and_then(|v| v.as_str()) else { continue };
        if t.to_ascii_lowercase() == ticker_l {
            let cik_num = entry.get("cik_str").and_then(|v| v.as_u64()).ok_or_else(|| anyhow!("missing cik_str for ticker {ticker}"))?;
            let title = entry.get("title").and_then(|v| v.as_str()).unwrap_or(ticker).to_string();
            return Ok(SecCompany {
                cik: format!("{:010}", cik_num),
                ticker: t.to_string(),
                title,
            });
        }
    }

    Err(anyhow!("ticker not found in SEC ticker map: {ticker}"))
}

pub fn fetch_latest_10k_csv(ticker: &str, out: impl AsRef<Path>) -> Result<()> {
    let company = fetch_company_by_ticker(ticker)?;
    let submissions_url = format!("https://data.sec.gov/submissions/CIK{}.json", company.cik);
    let submissions: Value = ureq::get(&submissions_url)
        .set("User-Agent", "quick-search/0.1 contact@example.com")
        .call()
        .with_context(|| format!("fetch SEC submissions for {}", company.ticker))?
        .into_json()
        .context("parse SEC submissions JSON")?;

    let recent = submissions.get("filings").and_then(|v| v.get("recent")).ok_or_else(|| anyhow!("missing filings.recent"))?;
    let forms = recent.get("form").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("missing recent.form"))?;
    let accessions = recent.get("accessionNumber").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("missing recent.accessionNumber"))?;
    let primary_docs = recent.get("primaryDocument").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("missing recent.primaryDocument"))?;
    let filing_dates = recent.get("filingDate").and_then(|v| v.as_array()).ok_or_else(|| anyhow!("missing recent.filingDate"))?;

    let idx = forms.iter().position(|v| v.as_str() == Some("10-K")).ok_or_else(|| anyhow!("no recent 10-K found for {}", company.ticker))?;
    let accession = accessions[idx].as_str().ok_or_else(|| anyhow!("bad accession"))?;
    let accession_nodash = accession.replace('-', "");
    let primary_doc = primary_docs[idx].as_str().ok_or_else(|| anyhow!("bad primary document"))?;
    let filing_date = filing_dates[idx].as_str().unwrap_or("");
    let year = filing_date.split('-').next().unwrap_or("0").parse::<i32>().unwrap_or(0);

    let cik_num = company.cik.trim_start_matches('0');
    let filing_url = format!("https://www.sec.gov/Archives/edgar/data/{}/{}/{}", cik_num, accession_nodash, primary_doc);
    let html = ureq::get(&filing_url)
        .set("User-Agent", "quick-search/0.1 contact@example.com")
        .call()
        .with_context(|| format!("fetch SEC filing {}", filing_url))?
        .into_string()
        .context("read SEC filing body")?;

    let text = from_read(Cursor::new(html.as_bytes()), 120);
    let chunks = chunk_text(&text, 1800);

    let mut w = Writer::from_path(out.as_ref())?;
    w.write_record(["id", "title", "abstract", "year", "journal", "authors", "doi"])?;

    for (i, chunk) in chunks.iter().enumerate() {
        let title = format!("{} latest 10-K chunk {}", company.ticker, i + 1);
        let doi = format!("sec:{}:{}:chunk{}", company.ticker, accession, i + 1);
        w.write_record([
            (i + 1).to_string(),
            title,
            clean_csv_text(chunk),
            year.to_string(),
            "SEC Form 10-K".to_string(),
            company.title.clone(),
            doi,
        ])?;
    }

    w.flush()?;
    Ok(())
}

fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut cur = String::new();

    for para in text.lines().map(str::trim).filter(|s| s.len() > 80 && is_content_line(s)) {
        if cur.len() + para.len() + 1 > max_chars && !cur.is_empty() {
            chunks.push(cur.trim().to_string());
            cur.clear();
        }
        cur.push_str(para);
        cur.push('\n');
        if chunks.len() >= 250 {
            break;
        }
    }

    if !cur.trim().is_empty() {
        chunks.push(cur.trim().to_string());
    }

    chunks
}

fn is_content_line(s: &str) -> bool {
    let alnum = s.chars().filter(|c| c.is_alphanumeric()).count();
    let total = s.chars().count().max(1);
    alnum * 100 / total >= 55
        && !s.contains("────")
        && !s.contains("┬")
        && !s.contains("┼")
        && !s.contains("Table of Contents")
}

fn clean_csv_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
