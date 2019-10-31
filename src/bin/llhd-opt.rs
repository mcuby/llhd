// Copyright (c) 2017-2019 Fabian Schuiki

#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

use clap::Arg;
use llhd::{assembly::parse_module, verifier::Verifier};
use std::{
    fs::File,
    io::{BufWriter, Read},
    result::Result,
};

fn main() {
    match main_inner() {
        Ok(_) => (),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn main_inner() -> Result<(), String> {
    let matches = app_from_crate!()
        .about("Optimizes LLHD assembly.")
        .arg(
            Arg::with_name("verbosity")
                .short("v")
                .multiple(true)
                .help(HELP_VERBOSITY.lines().next().unwrap())
                .long_help(HELP_VERBOSITY),
        )
        .arg(
            Arg::with_name("input")
                .help("LLHD file to optimize")
                .required(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .takes_value(true)
                .help("File to write output to; stdout if omitted"),
        )
        .arg(
            Arg::with_name("time-passes")
                .short("t")
                .long("time")
                .help("Print execution time statistics per pass"),
        )
        .get_matches();

    // Configure the logger.
    let verbose = std::cmp::max(1, matches.occurrences_of("verbosity") as usize) - 1;
    let quiet = !matches.is_present("verbosity");
    stderrlog::new()
        .module("llhd")
        .module("llhd_opt")
        .quiet(quiet)
        .verbosity(verbose)
        .init()
        .unwrap();

    // Read the input.
    let t0 = time::precise_time_ns();
    let mut module = {
        let path = matches.value_of("input").unwrap();
        let mut input = File::open(path).map_err(|e| format!("{}", e))?;
        let mut contents = String::new();
        input
            .read_to_string(&mut contents)
            .map_err(|e| format!("{}", e))?;
        let module = parse_module(&contents).map_err(|e| format!("{}", e))?;
        let mut verifier = Verifier::new();
        verifier.verify_module(&module);
        verifier.finish().map_err(|errs| format!("{}", errs))?;
        module
    };

    // Apply optimization pass.
    let t1 = time::precise_time_ns();
    llhd::pass::const_folding::run_on_module(&mut module);
    let t2 = time::precise_time_ns();
    // GCSE will go here
    let t3 = t2;
    llhd::pass::dead_code_elim::run_on_module(&mut module);
    let t4 = time::precise_time_ns();

    // Verify modified module.
    let mut verifier = Verifier::new();
    verifier.verify_module(&module);
    verifier
        .finish()
        .map_err(|errs| format!("Verification failed after optimization:\n{}", errs))?;
    let t5 = time::precise_time_ns();

    // Write the output.
    if let Some(path) = matches.value_of("output") {
        let output = File::create(path).map_err(|e| format!("{}", e))?;
        let output = BufWriter::with_capacity(1 << 20, output);
        llhd::assembly::write_module(output, &module);
    } else {
        llhd::assembly::write_module(std::io::stdout().lock(), &module);
    }
    let t6 = time::precise_time_ns();

    // Print execution time statistics if requested by the user.
    if matches.is_present("time-passes") {
        eprintln!("Execution Time Statistics:");
        eprintln!("  Parse:   {:8.3} ms", (t1 - t0) as f64 * 1.0e-6);
        eprintln!("  CF:      {:8.3} ms", (t2 - t1) as f64 * 1.0e-6);
        // GCSE will go here
        eprintln!("  DCE:     {:8.3} ms", (t4 - t3) as f64 * 1.0e-6);
        eprintln!("  Verify:  {:8.3} ms", (t5 - t4) as f64 * 1.0e-6);
        eprintln!("  Output:  {:8.3} ms", (t6 - t5) as f64 * 1.0e-6);
        eprintln!("  Total:   {:8.3} ms", (t6 - t0) as f64 * 1.0e-6);
    }

    Ok(())
}

static HELP_VERBOSITY: &str = "Increase message verbosity

This option can be specified multiple times to increase the level of verbosity \
in the output:

-v      Only print errors
-vv     Also print warnings
-vvv    Also print info messages
-vvvv   Also print debug messages
-vvvvv  Also print detailed tracing messages
";
