#[cfg(target_os = "linux")]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use mdc_replay::{
        recovery::{RunLimits, RunManifest, RunStore},
        LagAnchor, Profile, Symbol, TimestampUnit,
    };
    use std::{
        fs::OpenOptions,
        io::{Read, Write},
    };
    let a: Vec<_> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
  Some("create") if a.len()==9=>{
   let symbol=match a[4].as_str(){"BTCUSDT"=>Symbol::BtcUsdt,"ETHUSDT"=>Symbol::EthUsdt,_=>return Err("unsupported symbol".into())};
   let unit=match a[5].as_str(){"ms"=>TimestampUnit::Milliseconds,"us"=>TimestampUnit::Microseconds,_=>return Err("explicit ms/us required".into())};
   let anchor=match a[7].as_str(){"close"=>LagAnchor::ReportedClose,"end"=>LagAnchor::MinuteEnd,_=>return Err("close/end anchor required".into())};
   let profile=Profile::new(symbol,unit,a[6].parse()?,anchor)?;
   let mut source=Vec::new();std::fs::File::open(&a[3])?.take(4*1024*1024+1).read_to_end(&mut source)?;
   if a[8].len()>8192{return Err("cutoff argument too large".into());}
   let plan=a[8].split(',').map(str::parse).collect::<Result<Vec<u64>,_>>()?;
   let manifest=RunManifest::new(&source,profile,plan,RunLimits::default())?;
   let store=RunStore::create(&a[2],&source,manifest)?;println!("MDC_REPLAY_RUN=CREATED durable_through={}",store.durable_count()?);
  }
  Some("step") if a.len()==4=>{let mut s=RunStore::open(&a[2],None)?;let b=s.step(a[3].parse()?)?;println!("MDC_REPLAY_RUN=DURABLE index={} snapshot_sha256={}",b.index(),hex(&b.snapshot_sha256()));}
  Some("finish") if a.len()==3=>{
   let mut s=RunStore::open(&a[2],None)?;
   while let Some(index)=s.next_index()? {s.step(index)?;}
   println!("MDC_REPLAY_RUN=COMPLETE durable_through={}",s.durable_count()?);
  }
  Some("status") if a.len()==3=>{let s=RunStore::open(&a[2],None)?;println!("MDC_REPLAY_RUN=OPEN durable_through={} next={:?} run_id={}",s.durable_count()?,s.next_index()?,hex(&s.manifest().run_id()));}
  Some("export") if a.len()==6=>{
   let s=RunStore::open(&a[2],None)?;let b=s.lookup(a[3].parse()?)?.ok_or("index is not committed")?;
   OpenOptions::new().write(true).create_new(true).open(&a[4])?.write_all(b.report())?;
   OpenOptions::new().write(true).create_new(true).open(&a[5])?.write_all(b.snapshot())?;
   println!("MDC_REPLAY_RUN=EXPORTED index={}",b.index());
  }
  _=>return Err("create DIR CSV SYMBOL ms|us LAG_MS close|end CUTOFF_US,...; step DIR INDEX; finish DIR; status DIR; export DIR INDEX REPORT SNAPSHOT".into()),
 }
    Ok(())
}
#[cfg(target_os = "linux")]
fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(b.len() * 2);
    for v in b {
        write!(&mut out, "{v:02x}").expect("String formatting");
    }
    out
}
fn main() {
    #[cfg(target_os = "linux")]
    if let Err(e) = run() {
        eprintln!("MDC_REPLAY_RUN=ERROR {e}");
        std::process::exit(1);
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("MDC_REPLAY_RUN=ERROR Linux recovery profile required");
        std::process::exit(1);
    }
}
