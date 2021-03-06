extern crate clap;
extern crate console;
extern crate reqwest;
extern crate indicatif;

use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::copy;
use reqwest::{Client, Url, UrlError};
use reqwest::header::{Range, ByteRangeSpec, ContentLength, ContentType, AcceptRanges, RangeUnit};
use indicatif::{ProgressBar, ProgressStyle, HumanBytes};
use clap::{Arg, App};
use console::style;

fn parse_url(url: &str) -> Result<Url, UrlError> {
    match Url::parse(url) {
        Ok(url) => Ok(url),
        Err(error) if error == UrlError::RelativeUrlWithoutBase => {
            let url_with_base = format!("{}{}", "http://", url);
            Url::parse(url_with_base.as_str())
        }
        Err(error) => return Err(error),
    }

}

fn create_progress_bar(quiet_mode: bool, msg: &str, length: Option<u64>) -> ProgressBar {
    let bar = match quiet_mode {
        true => ProgressBar::hidden(),
        false => {
            match length {
                Some(len) => ProgressBar::new(len),
                None => ProgressBar::new_spinner(),
            }
        }
    };

    bar.set_message(msg);
    match length.is_some() {
        true => bar
            .set_style(ProgressStyle::default_bar()
                .template("{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} eta: {eta}")
                .progress_chars("=> ")),
        false => bar.set_style(ProgressStyle::default_spinner()),
    };

    bar
}


fn download(target: &str, quiet_mode: bool, filename: Option<&str>, resume_download: bool) -> Result<(), Box<::std::error::Error>> {
    
    let fname = match filename {
            Some(name) => name,
            None => target.split("/").last().unwrap(),
        };

    // parse url
    let url = parse_url(target)?;
    let client = Client::new().unwrap();
    let mut resp = match resume_download {
        true => {
            let req_headers = client.head(parse_url(target)?)?.send()?.headers().clone();
            match req_headers.get::<AcceptRanges>() {
                Some(header) => {
                    if header.contains(&RangeUnit::Bytes) {
                        let byte_count = match fs::metadata(fname) {
                            Ok(metadata) => metadata.len(),
                            Err(_) => 0u64,
                        };
                        // if byte_count is zero don't pass range header
                        match byte_count {
                            0 => client.get(url)?.send()?,
                            _ => {
                                let byte_range = Range::Bytes(vec![ByteRangeSpec::AllFrom(byte_count)]);
                                client.get(url)?.header(byte_range).send()?
                            },
                        }
                    } else {
                        client.get(url)?.send()?
                    } 
                },
                None => client.get(url)?.send()?
            }

            //client.get(url)?.send()?

        },
        false => client.get(url)?.send().unwrap(),
    };
    print(format!("HTTP request sent... {}",
                  style(format!("{}", resp.status())).green()),
          quiet_mode);
    if resp.status().is_success() {

        let headers = resp.headers().clone();
        let ct_len = headers.get::<ContentLength>().map(|ct_len| **ct_len);

        let ct_type = headers.get::<ContentType>().unwrap();

        match ct_len {
            Some(len) => {
                print(format!("Length: {} ({})",
                      style(len).green(),
                      style(format!("{}", HumanBytes(len))).red()),
                    quiet_mode);
            },
            None => {
                print(format!("Length: {}", style("unknown").red()), quiet_mode); 
            },
        }

        print(format!("Type: {}", style(ct_type).green()), quiet_mode);

        print(format!("Saving to: {}", style(fname).green()), quiet_mode);

        let chunk_size = match ct_len {
            Some(x) => x as usize / 99,
            None => 1024usize, // default chunk size
        };

        let mut buf = Vec::new();

        let bar = create_progress_bar(quiet_mode, fname, ct_len);

        loop {
            let mut buffer = vec![0; chunk_size];
            let bcount = resp.read(&mut buffer[..]).unwrap();
            buffer.truncate(bcount);
            if !buffer.is_empty() {
                buf.extend(buffer.into_boxed_slice()
                               .into_vec()
                               .iter()
                               .cloned());
                bar.inc(bcount as u64);
            } else {
                break;
            }
        }

        bar.finish();

        save_to_file(&mut buf, fname)?;
    }

    Ok(())

}

fn save_to_file(contents: &mut Vec<u8>, fname: &str) -> Result<(), std::io::Error> {
    let mut file = File::create(fname).unwrap();
    copy(&mut contents.as_slice(), &mut file).unwrap();
    Ok(())

}

fn print(string: String, quiet_mode: bool) {
    // print if not in quiet mode
    if !quiet_mode {
        println!("{}", string);
    }
}

fn main() {
    let args = App::new("Rget")
        .version("0.1.0")
        .author("Matt Gathu <mattgathu@gmail.com>")
        .about("wget clone written in Rust")
        .arg(Arg::with_name("quiet")
                 .short("q")
                 .long("quiet")
                 .help("quiet (no output)")
                 .required(false)
                 .takes_value(false))
        .arg(Arg::with_name("continue")
             .short("c")
             .long("continue")
             .help("resume getting a partially-downloaded file")
             .required(false)
             .takes_value(false))
        .arg(Arg::with_name("FILE")
             .short("O")
             .long("output-document")
             .help("write documents to FILE")
             .required(false)
             .takes_value(true))
        .arg(Arg::with_name("URL")
                 .required(true)
                 .takes_value(true)
                 .index(1)
                 .help("url to download"))
        .get_matches();
    let url = args.value_of("URL").unwrap();
    let quiet_mode = args.is_present("quiet");
    let resume_download = args.is_present("continue");
    let file_name = args.value_of("FILE");
    match download(url, quiet_mode, file_name, resume_download) {
        Ok(_) => {},
        Err(e) => print(format!("Got error: {}", e.description()), quiet_mode),
    }
}
