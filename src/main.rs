use std::{io::{Cursor, Write}, net::Ipv4Addr, path::PathBuf};

use anyhow::Result;
use base64::{prelude::BASE64_STANDARD, Engine};
use image::{ImageOutputFormat, ColorType};
use serde::{Deserialize, Serialize};
use clap::Parser;

#[derive(Parser)]
struct Args {
    /// llama.cpp server host
    #[clap(long, default_value = "127.0.0.1")]
    host: Ipv4Addr,

    /// llama.cpp server port
    #[clap(long, default_value = "7001")]
    port: u16,

    /// Initial prompt in format 
    /// `<system> USER: <user> ASSISTANT: <empty or handwritten assistant response>`
    #[clap(short, long, default_value = "\
Assistant is skillful in writing long and detailed description to images.
USER: [img-1] Describe the image.
ASSISTANT:"
    )]
    prompt: String, 

    /// Has priority over `--prompt`.
    /// Read initial prompt from file.
    #[clap(long)]
    prompt_file: Option<PathBuf>,

    /// Start in interactive chat mode
    #[clap(short, long)]
    interactive: bool,

    /// Copy response to clipboard (not in interactive mode)
    #[clap(short, long)]
    copy_back: bool,

    /// Sampling temperature.
    #[clap(short, long, default_value = "0.5")]
    temperature: f32,

    /// Token predict limit.
    #[clap(short, long, default_value = "1024")]
    n_predict: u32,

}

fn main() -> Result<()> {
    let args = Args::parse();
    
    let mut clip = arboard::Clipboard::new()?;
    let img = clip.get_image()?;
    let mut buf = Cursor::new(vec![]);

    image::write_buffer_with_format(
        &mut buf, 
        &img.bytes, 
        img.width as _, 
        img.height as _, 
        ColorType::Rgba8, 
        ImageOutputFormat::Png
    )?;

    let b64 = BASE64_STANDARD.encode(buf.into_inner());

    let prompt = args.prompt_file
        .map(std::fs::read_to_string)
        .transpose()?
        .unwrap_or(args.prompt);

    let mut req = Request {
        prompt,
        temperature: args.temperature,
        n_predict: args.n_predict,
        cache_prompt: true,
        image_data: vec![ImData { data: b64, id: 1 }],
        stop: vec!["USER:".to_string()]
    };

    let endpoint = format!("http://{}:{}/completion", args.host, args.port);
    let request = move |req: &Request| -> Result<String> {    
        let resp: Response = ureq::post(&endpoint)
            .send_json(req)?
            .into_json()?;

        Ok(resp.content)
    };

    let resp = request(&req)?;

    println!("{}{}", req.prompt, resp);
    req.prompt.push_str(&resp);

    if args.interactive {
        let mut line = String::new();
        loop {
            line.clear();
            print!("USER: ");
            std::io::stdout().flush()?;
            std::io::stdin().read_line(&mut line)?;
            
            req.prompt.push_str(&format!("USER: {line}\nASSISTANT:"));
            let resp = request(&req)?;
            println!("ASSISTANT: {resp}");
            req.prompt.push_str(&resp);
        }
    } else if args.copy_back {
        clip.set_text(resp)?;
    }

    Ok(())
}

#[derive(Serialize)]
struct Request {
    prompt: String,
    temperature: f32,
    n_predict: u32,
    cache_prompt: bool,
    image_data: Vec<ImData>,
    stop: Vec<String>,
}

#[derive(Serialize)]
struct ImData {
    data: String,
    id: u32,
}

#[derive(Deserialize)]
struct Response {
    content: String
}

