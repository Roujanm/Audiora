//! Vendor tool for Roujan Digital Agency: create keys and issue licenses.
//!
//!   audiora-license keygen --out private.key
//!   audiora-license issue  --key private.key --edition studio --to "Name"
//!                          [--serial R-0042] [--days 365] [--feature F]... [--out license.json]
//!   audiora-license verify --pub <hex> --file license.json [--serial R-0042]
//!
//! Keep `private.key` offline and out of version control.

use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use audiora_license::{
    generate_signing_key, hex_encode, issue, public_key_from_hex, signing_key_from_hex, verify, Claims,
    PRODUCT,
};

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == name).map(|w| w[1].clone())
}

fn flags(args: &[String], name: &str) -> Vec<String> {
    args.windows(2).filter(|w| w[0] == name).map(|w| w[1].clone()).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("keygen") => {
            let out = flag(args, "--out").ok_or("--out FILE is required")?;
            if std::path::Path::new(&out).exists() {
                return Err(format!("{out} already exists; refusing to overwrite a key"));
            }
            let key = generate_signing_key().map_err(|e| e.to_string())?;
            std::fs::write(&out, hex_encode(&key.to_bytes())).map_err(|e| e.to_string())?;
            println!("private key written to {out} (keep it offline)");
            println!("public key (embed with AUDIORA_LICENSE_PUBKEY at build time):");
            println!("{}", hex_encode(key.verifying_key().as_bytes()));
            Ok(())
        }
        Some("issue") => {
            let key_hex = std::fs::read_to_string(flag(args, "--key").ok_or("--key FILE is required")?)
                .map_err(|e| e.to_string())?;
            let key = signing_key_from_hex(&key_hex).ok_or("key file is not a valid private key")?;
            let issued_at = now();
            let claims = Claims {
                product: PRODUCT.into(),
                edition: flag(args, "--edition").ok_or("--edition play|studio is required")?,
                issued_to: flag(args, "--to").ok_or("--to NAME is required")?,
                device_serial: flag(args, "--serial"),
                issued_at,
                expires_at: flag(args, "--days")
                    .map(|d| d.parse::<u64>().map_err(|_| "--days must be a number".to_string()))
                    .transpose()?
                    .map(|d| issued_at + d * 86_400),
                features: flags(args, "--feature"),
            };
            if !matches!(claims.edition.as_str(), "play" | "studio") {
                return Err("edition must be play or studio".into());
            }
            let text = issue(&key, &claims);
            match flag(args, "--out") {
                Some(path) => std::fs::write(&path, text).map_err(|e| e.to_string())?,
                None => println!("{text}"),
            }
            Ok(())
        }
        Some("verify") => {
            let public = public_key_from_hex(&flag(args, "--pub").ok_or("--pub HEX is required")?)
                .ok_or("invalid public key")?;
            let text = std::fs::read_to_string(flag(args, "--file").ok_or("--file FILE is required")?)
                .map_err(|e| e.to_string())?;
            let serials: Vec<String> = flags(args, "--serial");
            let claims = verify(&text, &public, now(), &serials).map_err(|e| e.to_string())?;
            println!("valid: {} edition, issued to {}", claims.edition, claims.issued_to);
            Ok(())
        }
        _ => Err("usage: audiora-license keygen|issue|verify (see source header)".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
