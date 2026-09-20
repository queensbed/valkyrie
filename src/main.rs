use std::fs::{create_dir_all, read_to_string, write};
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
    winner_id: Option<usize>,
    payout: u32,
}

#[derive(Clone, Debug)]
struct MatchEvent {
    round: usize,
    title: String,
    detail: String,
}

#[derive(Clone)]
struct AppState {
    agents: Arc<Mutex<Vec<Agent>>>,
    tournament: Arc<Mutex<Tournament>>,
    events: Arc<Mutex<Vec<MatchEvent>>>,
    storage_path: Option<Arc<String>>,
}

fn main() {
    let storage_path = Arc::new("data/valkyrie.state".to_string());
    let (agents, tournament, events) = load_state(&storage_path);
    let state = AppState {
        agents: Arc::new(Mutex::new(agents)),
        tournament: Arc::new(Mutex::new(tournament)),
        events: Arc::new(Mutex::new(events)),
        storage_path: Some(storage_path),
    };
    let listener = TcpListener::bind("0.0.0.0:8080").expect("bind port 8080");
    println!("Valkyrie Cup listening on http://localhost:8080");
    for stream in listener.incoming().flatten() {
        let state = state.clone();
        thread::spawn(move || handle_connection(stream, state));
    }
}

fn default_state() -> (Vec<Agent>, Tournament, Vec<MatchEvent>) {
    (
        vec![
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
        ],
        Tournament {
            name: "Valkyrie Cup · Week 01".into(),
            status: "LIVE".into(),
            prize: "2,500 credits".into(),
            agents: vec![1, 2],
            winner_id: None,
            payout: 0,
        },
        vec![
            MatchEvent {
                round: 3,
                title: "Pressing Phoenix takes the lead".into(),
                detail: "A two-goal burst lifts the current leader to 84 rating.".into(),
            },
            MatchEvent {
                round: 2,
                title: "Calm Current controls midfield".into(),
                detail: "Possession play earns a second tournament win.".into(),
            },
        ],
    )
}

fn load_state(path: &str) -> (Vec<Agent>, Tournament, Vec<MatchEvent>) {
    let Ok(raw) = read_to_string(path) else {
        return default_state();
    };
    let (mut agents, mut tournament, mut events) = (Vec::new(), None, Vec::new());
    for line in raw.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        match parts.first().copied() {
            Some("agent") if parts.len() == 8 => agents.push(Agent {
                id: parts[1].parse().unwrap_or(0),
                name: parts[2].replace("\\p", "|"),
                style: parts[3].replace("\\p", "|"),
                prompt: parts[4].replace("\\p", "|"),
                rating: parts[5].parse().unwrap_or(60),
                wins: parts[6].parse().unwrap_or(0),
                goals: parts[7].parse().unwrap_or(0),
            }),
            Some("tournament") if parts.len() == 5 || parts.len() == 7 => {
                tournament = Some(Tournament {
                    name: parts[1].replace("\\p", "|"),
                    status: parts[2].replace("\\p", "|"),
                    prize: parts[3].replace("\\p", "|"),
                    agents: parts[4]
                        .split(',')
                        .filter_map(|id| id.parse().ok())
                        .collect(),
                    winner_id: parts.get(5).and_then(|id| id.parse().ok()),
                    payout: parts
                        .get(6)
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0),
                })
            }
            Some("event") if parts.len() == 4 => events.push(MatchEvent {
                round: parts[1].parse().unwrap_or(0),
                title: parts[2].replace("\\p", "|"),
                detail: parts[3].replace("\\p", "|"),
            }),
            _ => {}
        }
    }
    match (agents.is_empty(), tournament) {
        (false, Some(tournament)) => (agents, tournament, events),
        _ => default_state(),
    }
}

fn persist(state: &AppState) {
    let Some(path) = state.storage_path.as_deref() else {
        return;
    };
    let agents = state.agents.lock().unwrap();
    let tournament = state.tournament.lock().unwrap();
    let events = state.events.lock().unwrap();
    let clean = |value: &str| value.replace('|', "\\p").replace('\n', " ");
    let mut raw = agents
        .iter()
        .map(|agent| {
            format!(
                "agent|{}|{}|{}|{}|{}|{}|{}",
                agent.id,
                clean(&agent.name),
                clean(&agent.style),
                clean(&agent.prompt),
                agent.rating,
                agent.wins,
                agent.goals
            )
        })
        .collect::<Vec<_>>();
    raw.push(format!(
        "tournament|{}|{}|{}|{}|{}|{}",
        clean(&tournament.name),
        clean(&tournament.status),
        clean(&tournament.prize),
        tournament
            .agents
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        tournament
            .winner_id
            .map_or(String::new(), |id| id.to_string()),
        tournament.payout
    ));
    raw.extend(events.iter().map(|event| {
        format!(
            "event|{}|{}|{}",
            event.round,
            clean(&event.title),
            clean(&event.detail)
        )
    }));
    let _ = create_dir_all("data");
    let _ = write(path, raw.join("\n"));
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
        ("POST", "/api/tournament/round") => play_round(state),
        ("POST", "/api/tournament/settle") => settle_tournament(state),
        ("POST", "/api/tournament/reset") => reset_tournament(state),
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
    drop(agents);
    persist(state);
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
    let rating = agent.rating;
    drop(agents);
    persist(state);
    (
        "200 OK",
        "application/json",
        format!("{{\"rating\":{rating}}}"),
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
    drop(tournament);
    drop(agents);
    persist(state);
    (
        "200 OK",
        "application/json",
        "{\"message\":\"agent joined\"}".into(),
    )
}

fn play_round(state: &AppState) -> (&'static str, &'static str, String) {
    let tournament = state.tournament.lock().unwrap().clone();
    let mut agents = state.agents.lock().unwrap();
    let Some(winner_id) = tournament
        .agents
        .iter()
        .filter_map(|id| agents.iter().find(|agent| agent.id == *id))
        .max_by_key(|agent| agent.rating)
        .map(|agent| agent.id)
    else {
        return (
            "409 Conflict",
            "application/json",
            "{\"error\":\"no entrants\"}".into(),
        );
    };
    let winner = agents
        .iter_mut()
        .find(|agent| agent.id == winner_id)
        .unwrap();
    winner.wins += 1;
    winner.goals += 2;
    winner.rating = (winner.rating + 1).min(99);
    let winner_name = winner.name.clone();
    let winner_rating = winner.rating;
    drop(agents);
    let round = {
        let events = state.events.lock().unwrap();
        events.iter().map(|event| event.round).max().unwrap_or(0) + 1
    };
    let event = MatchEvent {
        round,
        title: format!("{winner_name} wins round {round}"),
        detail: format!("Two goals added; rating rises to {winner_rating}."),
    };
    state.events.lock().unwrap().insert(0, event);
    persist(state);
    (
        "200 OK",
        "application/json",
        format!(
            "{{\"winner\":\"{}\",\"goals\":2,\"round\":{}}}",
            escape(&winner_name),
            round
        ),
    )
}

fn settle_tournament(state: &AppState) -> (&'static str, &'static str, String) {
    let tournament_snapshot = state.tournament.lock().unwrap().clone();
    if tournament_snapshot.agents.is_empty() {
        return (
            "409 Conflict",
            "application/json",
            "{\"error\":\"no entrants\"}".into(),
        );
    }
    if tournament_snapshot.winner_id.is_some() {
        return (
            "409 Conflict",
            "application/json",
            "{\"error\":\"tournament already settled\"}".into(),
        );
    }
    let agents = state.agents.lock().unwrap();
    let Some(winner) = tournament_snapshot
        .agents
        .iter()
        .filter_map(|id| agents.iter().find(|agent| agent.id == *id))
        .max_by_key(|agent| (agent.wins, agent.goals, agent.rating))
    else {
        return (
            "409 Conflict",
            "application/json",
            "{\"error\":\"no valid entrants\"}".into(),
        );
    };
    let winner_id = winner.id;
    let winner_name = winner.name.clone();
    drop(agents);
    let mut tournament = state.tournament.lock().unwrap();
    tournament.status = "SETTLED".into();
    tournament.winner_id = Some(winner_id);
    tournament.payout = 2_500;
    let payout = tournament.payout;
    drop(tournament);
    persist(state);
    (
        "200 OK",
        "application/json",
        format!(
            "{{\"winner\":\"{}\",\"payout\":{}}}",
            escape(&winner_name),
            payout
        ),
    )
}

fn reset_tournament(state: &AppState) -> (&'static str, &'static str, String) {
    let mut agents = state.agents.lock().unwrap();
    for agent in agents.iter_mut() {
        agent.wins = 0;
        agent.goals = 0;
    }
    let mut tournament = state.tournament.lock().unwrap();
    let week = tournament
        .name
        .split("Week ")
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        + 1;
    tournament.name = format!("Valkyrie Cup · Week {week:02}");
    tournament.status = "LIVE".into();
    tournament.agents = agents.iter().map(|agent| agent.id).collect();
    tournament.winner_id = None;
    tournament.payout = 0;
    drop(tournament);
    drop(agents);
    let mut events = state.events.lock().unwrap();
    events.clear();
    events.push(MatchEvent {
        round: 1,
        title: format!("{} is live", state.tournament.lock().unwrap().name),
        detail: "Fresh standings, fresh strategies, same prize pool.".into(),
    });
    drop(events);
    persist(state);
    (
        "200 OK",
        "application/json",
        "{\"message\":\"new tournament week started\"}".into(),
    )
}

fn state_json(state: &AppState) -> String {
    let agents = state.agents.lock().unwrap();
    let tournament = state.tournament.lock().unwrap();
    let events = state.events.lock().unwrap();
    let agent_json = agents.iter().map(|agent| format!(
        "{{\"id\":{},\"name\":\"{}\",\"style\":\"{}\",\"prompt\":\"{}\",\"rating\":{},\"wins\":{},\"goals\":{}}}",
        agent.id, escape(&agent.name), escape(&agent.style), escape(&agent.prompt), agent.rating, agent.wins, agent.goals
    )).collect::<Vec<_>>().join(",");
    let event_json = events
        .iter()
        .map(|event| {
            format!(
                "{{\"round\":{},\"title\":\"{}\",\"detail\":\"{}\"}}",
                event.round,
                escape(&event.title),
                escape(&event.detail)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"tournament\":{{\"name\":\"{}\",\"status\":\"{}\",\"prize\":\"{}\",\"entrants\":{},\"settled\":{},\"winner\":{},\"payout\":{}}},\"agents\":[{}],\"events\":[{}]}}",
        escape(&tournament.name),
        tournament.status,
        escape(&tournament.prize),
        tournament.agents.len(),
        tournament.winner_id.is_some(),
        tournament.winner_id.map_or("null".into(), |id| id.to_string()),
        tournament.payout,
        agent_json,
        event_json
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
                winner_id: None,
                payout: 0,
            })),
            events: Arc::new(Mutex::new(vec![])),
            storage_path: None,
        };
        train_agent(r#"{"id":"1","prompt":"Press earlier"}"#, &state);
        assert_eq!(state.agents.lock().unwrap()[0].rating, 64);
    }

    #[test]
    fn round_rewards_highest_rated_entrant() {
        let state = AppState {
            agents: Arc::new(Mutex::new(vec![
                Agent {
                    id: 1,
                    name: "A".into(),
                    style: "B".into(),
                    prompt: "C".into(),
                    rating: 60,
                    wins: 0,
                    goals: 0,
                },
                Agent {
                    id: 2,
                    name: "Winner".into(),
                    style: "B".into(),
                    prompt: "C".into(),
                    rating: 80,
                    wins: 0,
                    goals: 0,
                },
            ])),
            tournament: Arc::new(Mutex::new(Tournament {
                name: "T".into(),
                status: "LIVE".into(),
                prize: "P".into(),
                agents: vec![1, 2],
                winner_id: None,
                payout: 0,
            })),
            events: Arc::new(Mutex::new(vec![])),
            storage_path: None,
        };
        play_round(&state);
        let agents = state.agents.lock().unwrap();
        assert_eq!(agents[1].wins, 1);
        assert_eq!(agents[1].goals, 2);
        assert_eq!(state.events.lock().unwrap()[0].round, 1);
    }
}
