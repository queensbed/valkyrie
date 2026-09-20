use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
struct Agent {
    id: usize,
    name: String,
    style: String,
    prompt: String,
    rating: u32,
    wins: u32,
    goals: u32,
}

#[derive(Clone, Debug)]
struct Tournament {
    name: String,
    status: String,
    prize: String,
    agents: Vec<usize>,
}

#[derive(Clone)]
struct AppState {
    agents: Arc<Mutex<Vec<Agent>>>,
    tournament: Arc<Mutex<Tournament>>,
}

fn main() {
    let state = AppState {
        agents: Arc::new(Mutex::new(vec![
            Agent {
                id: 1,
                name: "Pressing Phoenix".into(),
                style: "High press".into(),
                prompt: "Win the ball back within five seconds and attack the space.".into(),
                rating: 84,
                wins: 3,
                goals: 9,
            },
            Agent {
                id: 2,
                name: "Calm Current".into(),
                style: "Possession".into(),
                prompt: "Keep the ball moving and create the safest progressive pass.".into(),
                rating: 79,
                wins: 2,
                goals: 7,
            },
        ])),
        tournament: Arc::new(Mutex::new(Tournament {
            name: "Valkyrie Cup · Week 01".into(),
            status: "LIVE".into(),
            prize: "2,500 credits".into(),
            agents: vec![1, 2],
        })),
    };

    let listener = TcpListener::bind("0.0.0.0:8080").expect("bind port 8080");
    println!("Valkyrie Cup listening on http://localhost:8080");
    for stream in listener.incoming().flatten() {
        let state = state.clone();
        thread::spawn(move || handle_connection(stream, state));
    }
}

fn handle_connection(mut stream: TcpStream, state: AppState) {
    let mut buffer = [0; 16_384];
    let Ok(size) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..size]);
    let mut lines = request.lines();
    let Some(request_line) = lines.next() else {
        return;
    };
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");
    let (status, content_type, response) = route(method, path, body, &state);
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        response.len()
    );
    let _ = stream.write_all(format!("{headers}{response}").as_bytes());
}

fn route(
    method: &str,
    path: &str,
    body: &str,
    state: &AppState,
) -> (&'static str, &'static str, String) {
    match (method, path) {
        ("GET", "/") => (
            "200 OK",
            "text/html; charset=utf-8",
            include_str!("../public/index.html").into(),
        ),
        ("GET", "/api/state") => ("200 OK", "application/json", state_json(state)),
        ("POST", "/api/agents") => create_agent(body, state),
        ("POST", "/api/train") => train_agent(body, state),
        ("POST", "/api/tournament/join") => join_tournament(body, state),
        _ => (
            "404 Not Found",
            "application/json",
            "{\"error\":\"not found\"}".into(),
        ),
    }
}

fn create_agent(body: &str, state: &AppState) -> (&'static str, &'static str, String) {
    let name = value(body, "name").unwrap_or_else(|| "Unnamed Agent".into());
    let style = value(body, "style").unwrap_or_else(|| "Adaptive".into());
    let prompt = value(body, "prompt").unwrap_or_else(|| "Play intelligently.".into());
    let mut agents = state.agents.lock().unwrap();
    let id = agents.iter().map(|agent| agent.id).max().unwrap_or(0) + 1;
    agents.push(Agent {
        id,
        name,
        style,
        prompt,
        rating: 60,
        wins: 0,
        goals: 0,
    });
    (
        "201 Created",
        "application/json",
        "{\"message\":\"agent created\"}".into(),
    )
}

fn train_agent(body: &str, state: &AppState) -> (&'static str, &'static str, String) {
    let id = value(body, "id")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let prompt = value(body, "prompt").unwrap_or_else(|| "Make better decisions.".into());
    let mut agents = state.agents.lock().unwrap();
    let Some(agent) = agents.iter_mut().find(|agent| agent.id == id) else {
        return (
            "404 Not Found",
            "application/json",
            "{\"error\":\"agent not found\"}".into(),
        );
    };
    agent.prompt = prompt;
    agent.rating = (agent.rating + 4).min(99);
    (
        "200 OK",
        "application/json",
        format!("{{\"rating\":{}}}", agent.rating),
    )
}

fn join_tournament(body: &str, state: &AppState) -> (&'static str, &'static str, String) {
    let id = value(body, "id")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let agents = state.agents.lock().unwrap();
    if !agents.iter().any(|agent| agent.id == id) {
        return (
            "404 Not Found",
            "application/json",
            "{\"error\":\"agent not found\"}".into(),
        );
    }
    let mut tournament = state.tournament.lock().unwrap();
    if !tournament.agents.contains(&id) {
        tournament.agents.push(id);
    }
    (
        "200 OK",
        "application/json",
        "{\"message\":\"agent joined\"}".into(),
    )
}

fn state_json(state: &AppState) -> String {
    let agents = state.agents.lock().unwrap();
    let tournament = state.tournament.lock().unwrap();
    let agent_json = agents.iter().map(|agent| format!(
        "{{\"id\":{},\"name\":\"{}\",\"style\":\"{}\",\"prompt\":\"{}\",\"rating\":{},\"wins\":{},\"goals\":{}}}",
        agent.id, escape(&agent.name), escape(&agent.style), escape(&agent.prompt), agent.rating, agent.wins, agent.goals
    )).collect::<Vec<_>>().join(",");
    format!(
        "{{\"tournament\":{{\"name\":\"{}\",\"status\":\"{}\",\"prize\":\"{}\",\"entrants\":{}}},\"agents\":[{}]}}",
        escape(&tournament.name), tournament.status, escape(&tournament.prize), tournament.agents.len(), agent_json
    )
}

fn value(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let start = body.find(&marker)? + marker.len();
    let end = body[start..].find('"')? + start;
    Some(body[start..end].replace("\\\"", "\""))
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_reads_json_string() {
        assert_eq!(value(r#"{"name":"Nova"}"#, "name"), Some("Nova".into()));
    }

    #[test]
    fn training_increases_rating() {
        let state = AppState {
            agents: Arc::new(Mutex::new(vec![Agent {
                id: 1,
                name: "A".into(),
                style: "B".into(),
                prompt: "C".into(),
                rating: 60,
                wins: 0,
                goals: 0,
            }])),
            tournament: Arc::new(Mutex::new(Tournament {
                name: "T".into(),
                status: "LIVE".into(),
                prize: "P".into(),
                agents: vec![],
            })),
        };
        train_agent(r#"{"id":"1","prompt":"Press earlier"}"#, &state);
        assert_eq!(state.agents.lock().unwrap()[0].rating, 64);
    }
}
