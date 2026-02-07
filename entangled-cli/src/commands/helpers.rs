//! Shared command helpers.

use entangled::errors::Result;
use entangled::interface::Context;
use entangled::io::Transaction;

/// Common options for transaction-based commands.
pub struct TransactionOptions {
    pub force: bool,
    pub dry_run: bool,
    pub diff: bool,
    pub quiet: bool,
}

/// Runs a transaction with common option handling (diff, dry-run, force, quiet).
///
/// Returns Ok(()) after handling the transaction according to the options.
/// `verb` is used for display (e.g., "tangle", "stitch").
pub fn run_transaction(
    ctx: &mut Context,
    transaction: Transaction,
    options: &TransactionOptions,
    verb: &str,
) -> Result<()> {
    if transaction.is_empty() {
        if !options.quiet {
            println!("No files to {}.", verb);
        }
        return Ok(());
    }

    if options.diff {
        for diff in transaction.diffs() {
            println!("{}", diff);
        }
        return Ok(());
    }

    if options.dry_run {
        println!("Would perform {} actions:", transaction.len());
        for desc in transaction.describe() {
            println!("  {}", desc);
        }
        return Ok(());
    }

    if options.force {
        transaction.execute_force(&mut ctx.filedb)?;
    } else {
        transaction.execute(&mut ctx.filedb)?;
    }

    ctx.save_filedb()?;

    if !options.quiet {
        let past = match verb {
            "stitch" => "Stitched",
            "tangle" => "Tangled",
            _ => "Processed",
        };
        println!("{} {} files.", past, transaction.len());
    }

    Ok(())
}
