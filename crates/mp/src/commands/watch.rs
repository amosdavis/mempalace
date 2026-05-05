use anyhow::Result;
use mempalace::config::MempalaceConfig;
use mempalace::mining::mine_project;
use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub fn run(dir: &str, wing: &str, palace: Option<&str>) -> Result<()> {
    let cfg = MempalaceConfig::load();
    let palace_path = palace.unwrap_or(&cfg.palace_path).to_string();
    let dir_path = Path::new(dir).canonicalize()?;

    println!("Watching {} → wing:{}", dir_path.display(), wing);
    println!("Press Ctrl-C to stop.");

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default())?;
    watcher.watch(&dir_path, RecursiveMode::Recursive)?;

    let debounce = Duration::from_secs(3);
    let mut last_change: Option<Instant> = None;

    loop {
        // Drain events, note whether any relevant change occurred
        let has_event = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => is_relevant(&event),
            Ok(Err(e)) => { eprintln!("Watch error: {e}"); false }
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };

        if has_event {
            last_change = Some(Instant::now());
        }

        // Fire mine after debounce window expires
        if let Some(changed_at) = last_change {
            if changed_at.elapsed() >= debounce {
                last_change = None;
                print!("[mempalace watch] Change detected — mining… ");
                match mine_project(&dir_path, &palace_path, wing, None, false) {
                    Ok(s) => println!("{} files, {} chunks", s.files_processed, s.chunks_created),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
        }
    }

    Ok(())
}

fn is_relevant(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}
