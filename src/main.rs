
mod trace;
use iced_x86::{Decoder, DecoderOptions, FastFormatter, Instruction};
use std::{num::ParseIntError, path::PathBuf};
use std::io::prelude::*;
use std::fs::File;
use structopt::StructOpt;
use trace::Trace;

const HEXBYTES_COLUMN_BYTE_LENGTH: usize = 10;

fn parse_offset(src: &str) -> Result<u64, ParseIntError> {
    if src.len() > 2 && &src[0..2] == "0x" {
        u64::from_str_radix(&src[2..], 16)
    } else {
        u64::from_str_radix(src, 16)
    }
}

#[derive(StructOpt)]
#[structopt(about = "disassembles traces produced by libtrace")]
struct TracedisCli {
    #[structopt(long, short, parse(from_os_str), help = "path to trace file produced by the libtrace plugin")]
    trace: std::path::PathBuf,
    #[structopt(long, help = "offset of first 32-bit instruction executed", parse(try_from_str = parse_offset), required_ifs(&[("bits","64"), ("bits","32")]))]
    offset_32bit: Option<u64>,
    #[structopt(long, help = "offset of first 64-bit instruction executed", parse(try_from_str = parse_offset), required_if("bits","64"))]
    offset_64bit: Option<u64>,
    #[structopt(long, short, help = "either one of 16, 32, or 64", default_value = "64")]
    bits: u8,
    #[structopt(long, help = "handles traces that transition from 16-bit up to the bitness set with --bits",)]
    system_mode: bool,
}

fn disassemble(code: &[u8], bitness: u32, rip: u64) {
    let bytes = code;
    let mut decoder = Decoder::new(bitness, bytes, DecoderOptions::NONE);
    decoder.set_ip(rip);

    let mut formatter = FastFormatter::new();
    let mut output = String::new();
    let mut instruction = Instruction::default();
    while decoder.can_decode() {
        decoder.decode_out(&mut instruction);
        output.clear();
        formatter.format(&instruction, &mut output);
       
        let start_index = (instruction.ip() - rip) as usize;
        let instr_bytes = &bytes[start_index..start_index + instruction.len()];
        for b in instr_bytes.iter() {
            print!("{:02X}", b);
        }
        if instr_bytes.len() < HEXBYTES_COLUMN_BYTE_LENGTH {
            for _ in 0..HEXBYTES_COLUMN_BYTE_LENGTH - instr_bytes.len() {
                print!("  ");
            }
        }
        println!(" {}", output);
    }
}

fn read_trace(path: &PathBuf) -> Option<Trace> {
    let mut f = File::open(path).expect("failed to open file");
    let mut buffer = Vec::new();
    let result = f.read_to_end(&mut buffer);
    if result.is_err() {
        None
    } else {
        let trace = Trace::from(buffer);
        Some(trace)
    }
}

fn handle_system_mode(cli: &TracedisCli, trace: Trace)  -> Result<(), String>  {
    let mut trace_offset = 0;
    let mut bitness = 16;
    for insn in trace.into_iter() {
        let data = &insn.data[0..insn.size as usize];
        match cli.bits {
            16 => {}
            32 => {
                if bitness == 16 {
                    let o32 = cli.offset_32bit.unwrap();
                    if trace_offset == o32 {
                        bitness = 32
                    }
                }
            },
            64 => {
                if bitness == 16 {
                    let o32 = cli.offset_32bit.unwrap();
                    if trace_offset == o32 {
                        bitness = 32
                    }
                } else if bitness == 32 {
                    let o64 = cli.offset_64bit.unwrap();
                    if trace_offset == o64 {
                        bitness = 64
                    }
                }
            }
            _ => {
                panic!("Unknown bitness")
            }
        }
        print!("{:016X}   {:016X}   ", insn.vaddr, trace_offset);
        disassemble(&data, bitness, 0);
        trace_offset += insn.size as u64;
    }
    Ok(())
}

fn handle_linear(cli: &TracedisCli, trace: Trace) -> Result<(), String>  {
    let mut trace_offset = 0;
    for insn in trace.into_iter() {
        print!("{:016X} ", trace_offset);
        disassemble(&insn.data, cli.bits.into(), 0);
        trace_offset += insn.size as u64;
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let cli: TracedisCli = TracedisCli::from_args();
    if cli.bits != 16 && cli.bits != 32 && cli.bits != 64 {
        return Err(format!("Invalid bits value: {}.\nThe value must be one of either 16, 32, or 64\n", cli.bits));
    }

    let trace_path = &cli.trace;
    let maybe_trace = read_trace(trace_path);
    if maybe_trace.is_none() {
        return Err(format!("Failed to read trace file at {}", trace_path.to_str().unwrap()));
    }
    let trace = maybe_trace.unwrap();

    if cli.system_mode {
        handle_system_mode(&cli, trace)
    } else {
        handle_linear(&cli, trace)
    }
}