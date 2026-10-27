use clap::Parser;

/// Opinionated linter for Crossplane XRDs, Compositions and Functions.
#[derive(Parser, Debug)]
#[command(name = "xplint", version, about)]
struct Cli {}

fn greeting() -> &'static str {
    "xplint: scaffold only, no rules implemented yet"
}

fn main() {
    let _cli = Cli::parse();
    println!("{}", greeting());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_mentions_name() {
        assert!(greeting().contains("xplint"));
    }
}
