use regex_automata::DenseDFA;
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for (name, regex) in &[
        ("mention_regex", r"<@[A-Z0-9]{9}>"),
        ("channel_regex", r"<#[A-Z0-9]{9}\|(?P<n>.*?)>"),
        ("url_regex", r#"https?://.*?[\|>\s]"#),
    ] {
        let dfa_bytes = DenseDFA::new(regex)?.to_u16()?.to_bytes_native_endian()?;
        let contents = std::fs::read(name).unwrap_or_default();
        if contents != dfa_bytes {
            File::create(name)?.write_all(&dfa_bytes)?;
        }
    }

    Ok(())
}
