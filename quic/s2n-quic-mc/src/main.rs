use structopt::StructOpt;

mod report;
mod run;

#[derive(Debug, StructOpt)]
enum Args {
    Run(run::Run),
    Report(report::Report),
}

fn main() {
    match Args::from_args() {
        Args::Run(args) => args.run(),
        Args::Report(args) => args.run(),
    }
}
