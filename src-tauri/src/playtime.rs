use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use chrono::Utc;
use crate::database::Database;

const HEARTBEAT_INTERVAL_SECS: u64 = 15;
const CHECKPOINT_INTERVAL_SECS: u64 = 60;

#[derive(Debug)]
pub struct ActiveSession {
    pub session_id: String,
    pub game_id: String,
    pub install_id: Option<String>,
    pub process_pid: u32,
    pub accumulated_seconds: i64,
    pub last_heartbeat: Instant,
    pub last_checkpoint: Instant,
    pub started_at: String,
}

pub struct PlaytimeTracker {
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    db: Arc<Mutex<Database>>,
    running: Arc<Mutex<bool>>,
}

impl PlaytimeTracker {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        let tracker = Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            db,
            running: Arc::new(Mutex::new(false)),
        };
        
        // Recover any crashed sessions from previous run (non-fatal if it fails)
        let _ = tracker.recover_sessions();
        
        tracker
    }
    
    /// Recover sessions that were active when app crashed
    fn recover_sessions(&self) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        
        // Get all active sessions from DB
        let active_sessions = db.get_active_sessions().map_err(|e| e.to_string())?;
        
        for session in active_sessions {
            // Calculate lost time and update
            let now = Utc::now().to_rfc3339();
            
            // Mark session as recovered
            db.recover_session(&session.id, session.accumulated_seconds, &now)
                .map_err(|e| e.to_string())?;
            
            println!("Recovered session {} with {} seconds", session.id, session.accumulated_seconds);
        }
        
        Ok(())
    }
    
    /// Start tracking a new game session
    pub fn start_session(&self, game_id: &str, install_id: Option<&str>, pid: u32) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        
        // Create session in DB
        {
            let db = self.db.lock().map_err(|e| e.to_string())?;
            db.create_play_session(&session_id, game_id, install_id, &now_str)
                .map_err(|e| e.to_string())?;
            db.create_active_session(&session_id, game_id, pid, &now_str)
                .map_err(|e| e.to_string())?;
        }
        
        // Add to in-memory tracking
        {
            let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
            sessions.insert(session_id.clone(), ActiveSession {
                session_id: session_id.clone(),
                game_id: game_id.to_string(),
                install_id: install_id.map(|s| s.to_string()),
                process_pid: pid,
                accumulated_seconds: 0,
                last_heartbeat: Instant::now(),
                last_checkpoint: Instant::now(),
                started_at: now_str,
            });
        }
        
        // Start heartbeat loop if not running
        self.start_heartbeat_loop();
        
        Ok(session_id)
    }
    
    /// Start the background heartbeat loop
    fn start_heartbeat_loop(&self) {
        let mut running = self.running.lock().unwrap();
        if *running {
            return;
        }
        *running = true;
        drop(running);
        
        let sessions = Arc::clone(&self.sessions);
        let db = Arc::clone(&self.db);
        let running = Arc::clone(&self.running);
        
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
                
                let mut sessions_guard = sessions.lock().unwrap();
                
                if sessions_guard.is_empty() {
                    // No active sessions, stop the loop
                    let mut running_guard = running.lock().unwrap();
                    *running_guard = false;
                    break;
                }
                
                let mut to_remove = Vec::new();
                let now = Instant::now();
                let now_str = Utc::now().to_rfc3339();
                
                for (session_id, session) in sessions_guard.iter_mut() {
                    // Check if process is still running
                    if !is_process_running(session.process_pid) {
                        to_remove.push(session_id.clone());
                        continue;
                    }
                    
                    // Update accumulated time
                    let elapsed = now.duration_since(session.last_heartbeat);
                    session.accumulated_seconds += elapsed.as_secs() as i64;
                    session.last_heartbeat = now;
                    
                    // Update heartbeat in DB
                    {
                        let db_guard = db.lock().unwrap();
                        let _ = db_guard.update_active_session_heartbeat(
                            &session.session_id,
                            session.accumulated_seconds,
                            &now_str,
                        );
                    }
                    
                    // Checkpoint if needed
                    if now.duration_since(session.last_checkpoint).as_secs() >= CHECKPOINT_INTERVAL_SECS {
                        session.last_checkpoint = now;
                        
                        let db_guard = db.lock().unwrap();
                        
                        // Update play_session duration
                        let _ = db_guard.checkpoint_session(
                            &session.session_id,
                            session.accumulated_seconds,
                            &now_str,
                        );
                        
                        // Update game total playtime
                        let _ = db_guard.add_playtime(&session.game_id, session.accumulated_seconds);
                        
                        // Reset accumulated (it's been saved)
                        session.accumulated_seconds = 0;
                        
                        // Update active_session checkpoint time
                        let _ = db_guard.update_active_session_checkpoint(&session.session_id, &now_str);
                    }
                }
                
                // End sessions for processes that stopped
                for session_id in to_remove {
                    if let Some(session) = sessions_guard.remove(&session_id) {
                        let db_guard = db.lock().unwrap();
                        let _ = end_session_internal(&db_guard, &session, &now_str);
                    }
                }
            }
        });
    }
    
    /// Manually end a session
    pub fn end_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
        
        if let Some(session) = sessions.remove(session_id) {
            let now_str = Utc::now().to_rfc3339();
            let db = self.db.lock().map_err(|e| e.to_string())?;
            end_session_internal(&db, &session, &now_str).map_err(|e| e.to_string())?;
        }
        
        Ok(())
    }
    
    /// Get all active sessions
    pub fn get_active_sessions(&self) -> Vec<(String, String, i64)> {
        let sessions = self.sessions.lock().unwrap();
        sessions.values()
            .map(|s| (s.session_id.clone(), s.game_id.clone(), s.accumulated_seconds))
            .collect()
    }
}

fn end_session_internal(db: &Database, session: &ActiveSession, now_str: &str) -> Result<(), rusqlite::Error> {
    // Final playtime update
    db.add_playtime(&session.game_id, session.accumulated_seconds)?;
    
    // Calculate total duration
    let started = chrono::DateTime::parse_from_rfc3339(&session.started_at)
        .unwrap_or_else(|_| Utc::now().into());
    let ended = chrono::DateTime::parse_from_rfc3339(now_str)
        .unwrap_or_else(|_| Utc::now().into());
    let duration_seconds = (ended - started).num_seconds();
    
    // Complete the play session
    db.complete_session(&session.session_id, now_str, duration_seconds)?;
    
    // Remove active session
    db.delete_active_session(&session.session_id)?;
    
    // Update last played
    db.update_last_played(&session.game_id, now_str)?;
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> bool {
    use windows_sys::Win32::System::Threading::{OpenProcess, GetExitCodeProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::Foundation::CloseHandle;
    
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        
        // HANDLE is *mut c_void, null means invalid
        if handle.is_null() {
            return false;
        }
        
        let mut exit_code: u32 = 0;
        let result = GetExitCodeProcess(handle, &mut exit_code);
        
        CloseHandle(handle);
        
        // STILL_ACTIVE = 259, means process is still running
        result != 0 && exit_code == 259
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(pid: u32) -> bool {
    use std::process::Command;
    
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}