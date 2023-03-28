// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use self::db_dump::{dump_table, duplicate_objects_summary, list_tables, table_summary, StoreName};
use clap::Parser;
use std::path::{Path, PathBuf};
use sui_core::authority::authority_store_tables::AuthorityPerpetualTables;
use sui_core::checkpoints::CheckpointStore;
use sui_types::base_types::EpochId;
use typed_store::rocks::MetricConf;

pub mod db_dump;

#[derive(Parser)]
#[clap(rename_all = "kebab-case")]
pub enum DbToolCommand {
    ListTables,
    Dump(Dump),
    TableSummary(Dump),
    DuplicatesSummary,
    ResetDB,
}

#[derive(Parser)]
#[clap(rename_all = "kebab-case")]
pub struct Dump {
    /// The type of store to dump
    #[clap(long = "store", value_enum)]
    store_name: StoreName,
    /// The name of the table to dump
    #[clap(long = "table-name")]
    table_name: String,
    /// The size of page to dump. This is a u16
    #[clap(long = "page-size")]
    page_size: u16,
    /// The page number to dump
    #[clap(long = "page-num")]
    page_number: usize,

    // TODO: We should load this automatically from the system object in AuthorityPerpetualTables.
    // This is very difficult to do right now because you can't share code between
    // AuthorityPerpetualTables and AuthorityEpochTablesReadonly.
    /// The epoch to use when loading AuthorityEpochTables.
    #[clap(long = "epoch")]
    epoch: Option<EpochId>,
}

pub fn execute_db_tool_command(db_path: PathBuf, cmd: DbToolCommand) -> anyhow::Result<()> {
    match cmd {
        DbToolCommand::ListTables => print_db_all_tables(db_path),
        DbToolCommand::Dump(d) => print_all_entries(
            d.store_name,
            d.epoch,
            db_path,
            &d.table_name,
            d.page_size,
            d.page_number,
        ),
        DbToolCommand::TableSummary(d) => {
            print_db_table_summary(d.store_name, d.epoch, db_path, &d.table_name)
        }
        DbToolCommand::DuplicatesSummary => print_db_duplicates_summary(db_path),
        DbToolCommand::ResetDB => reset_db_to_genesis(&db_path),
    }
}

pub fn print_db_all_tables(db_path: PathBuf) -> anyhow::Result<()> {
    list_tables(db_path)?.iter().for_each(|t| println!("{}", t));
    Ok(())
}

pub fn print_db_duplicates_summary(db_path: PathBuf) -> anyhow::Result<()> {
    let (total_count, duplicate_count, total_bytes, duplicated_bytes) =
        duplicate_objects_summary(db_path);
    println!(
        "Total objects = {}, duplicated objects = {}, total bytes = {}, duplicated bytes = {}",
        total_count, duplicate_count, total_bytes, duplicated_bytes
    );
    Ok(())
}

pub fn reset_db_to_genesis(path: &Path) -> anyhow::Result<()> {
    let perpetual_db = AuthorityPerpetualTables::open_tables_read_write(path.join("store").join("perpetual").to_path_buf(), MetricConf::default(), None, None);
    perpetual_db.reset_db_for_execution_since_genesis()?;

    let checkpoint_db = CheckpointStore::open_tables_read_write(path.join("checkpoints").to_path_buf(), MetricConf::default(), None, None);
    checkpoint_db.reset_db_for_execution_since_genesis()?;
    Ok(())
}

pub fn print_db_table_summary(
    store: StoreName,
    epoch: Option<EpochId>,
    path: PathBuf,
    table_name: &str,
) -> anyhow::Result<()> {
    let summary = table_summary(store, epoch, path, table_name)?;
    let quantiles = vec![25, 50, 75, 90, 99];
    println!(
        "Total num keys = {}, total key bytes = {}, total value bytes = {}",
        summary.num_keys, summary.key_bytes_total, summary.value_bytes_total
    );
    println!("Key size distribution:\n");
    quantiles.iter().for_each(|q| {
        println!(
            "p{:?} -> {:?} bytes\n",
            q,
            summary.key_hist.value_at_quantile(*q as f64 / 100.0)
        );
    });
    println!("Value size distribution:\n");
    quantiles.iter().for_each(|q| {
        println!(
            "p{:?} -> {:?} bytes\n",
            q,
            summary.value_hist.value_at_quantile(*q as f64 / 100.0)
        );
    });
    Ok(())
}

pub fn print_all_entries(
    store: StoreName,
    epoch: Option<EpochId>,
    path: PathBuf,
    table_name: &str,
    page_size: u16,
    page_number: usize,
) -> anyhow::Result<()> {
    for (k, v) in dump_table(store, epoch, path, table_name, page_size, page_number)? {
        println!("{:>100?}: {:?}", k, v);
    }
    Ok(())
}
