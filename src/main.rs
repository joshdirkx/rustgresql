use ratatui::{
    backend::{CrosstermBackend},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Table, Row, Cell},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color},
    Terminal,
};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use tokio_postgres::NoTls;
use std::{env, error::Error, io};
use dotenv::dotenv;

// Function to fetch connection details from environment variables
fn get_connection_string() -> Result<String, Box<dyn Error>> {
    dotenv().ok(); // Load .env file

    // Read environment variables
    let user = env::var("POSTGRES_USER")?;
    let password = env::var("POSTGRES_PASSWORD")?;
    let host = env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());

    Ok(format!(
        "host={} port={} user={} password={}",
        host, port, user, password
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActivePane {
    Databases,
    Tables,
    Main,
    QueryInput,
}

struct AppState {
    databases: Vec<String>,
    selected_database: Option<usize>,
    tables: Vec<String>,
    selected_table: Option<usize>,
    query: String,
    query_result: Vec<Vec<String>>, // Store query result as a 2D vector
    active_pane: ActivePane,
}

impl AppState {
    fn new(databases: Vec<String>) -> Self {
        Self {
            databases,
            selected_database: Some(0),
            tables: vec![],
            selected_table: Some(0),
            query: String::new(),
            query_result: vec![],
            active_pane: ActivePane::Databases,
        }
    }

    fn next_database(&mut self) {
        if let Some(selected) = self.selected_database {
            if selected < self.databases.len() - 1 {
                self.selected_database = Some(selected + 1);
            }
        }
    }

    fn previous_database(&mut self) {
        if let Some(selected) = self.selected_database {
            if selected > 0 {
                self.selected_database = Some(selected - 1);
            }
        }
    }

    fn next_table(&mut self) {
        if let Some(selected) = self.selected_table {
            if selected < self.tables.len() - 1 {
                self.selected_table = Some(selected + 1);
            }
        }
    }

    fn previous_table(&mut self) {
        if let Some(selected) = self.selected_table {
            if selected > 0 {
                self.selected_table = Some(selected - 1);
            }
        }
    }

    fn set_tables(&mut self, tables: Vec<String>) {
        self.tables = tables;
        self.selected_table = Some(0);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let connection_string = get_connection_string()?;
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    let rows = client.query("SELECT datname FROM pg_database WHERE datistemplate = false", &[]).await?;
    let databases: Vec<String> = rows.iter().map(|row| row.get(0)).collect();

    let mut app_state = AppState::new(databases);

    if let Some(selected) = app_state.selected_database {
        let db_name = &app_state.databases[selected];
        let tables = fetch_tables(db_name).await?;
        app_state.set_tables(tables);
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
                .split(size);

            let vertical_chunks_left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(horizontal_chunks[0]);

            let vertical_chunks_right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(70),
                    Constraint::Percentage(20),
                    Constraint::Percentage(10),
                ])
                .split(horizontal_chunks[1]);

            let db_items: Vec<ListItem> = app_state
                .databases
                .iter()
                .map(|db| ListItem::new(db.clone()))
                .collect();

            let mut db_list_state = ListState::default();
            db_list_state.select(app_state.selected_database);

            let db_sidebar = List::new(db_items)
                .block(Block::default()
                    .title("Databases")
                    .borders(Borders::ALL)
                    .style(if app_state.active_pane == ActivePane::Databases {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }))
                .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black))
                .highlight_symbol("> ");

            let table_items: Vec<ListItem> = app_state
                .tables
                .iter()
                .map(|table| ListItem::new(table.clone()))
                .collect();

            let mut table_list_state = ListState::default();
            table_list_state.select(app_state.selected_table);

            let table_sidebar = List::new(table_items)
                .block(Block::default()
                    .title("Tables")
                    .borders(Borders::ALL)
                    .style(if app_state.active_pane == ActivePane::Tables {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }))
                .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black))
                .highlight_symbol("> ");

            let query_result_table = Table::new(
    app_state
        .query_result
        .iter()
        .map(|row| Row::new(row.iter().map(|cell| Cell::from(cell.clone())))),
    vec![Constraint::Min(10); app_state.query_result.first().map_or(0, |row| row.len())], // Set column widths dynamically
)
.block(Block::default().title("Query Results").borders(Borders::ALL));


            let query_input = Paragraph::new(app_state.query.clone())
                .block(Block::default()
                    .title("Enter Query")
                    .borders(Borders::ALL)
                    .style(if app_state.active_pane == ActivePane::QueryInput {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }));

            f.render_stateful_widget(db_sidebar, vertical_chunks_left[0], &mut db_list_state);
            f.render_stateful_widget(table_sidebar, vertical_chunks_left[1], &mut table_list_state);
            f.render_widget(query_result_table, vertical_chunks_right[0]);
            f.render_widget(query_input, vertical_chunks_right[2]);
        })?;

        if let Event::Key(key) = event::read()? {
            match (key.code, key.modifiers) {
                (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                    app_state.active_pane = ActivePane::Databases;
                }
                (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                    app_state.active_pane = ActivePane::Tables;
                }
                (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                    app_state.active_pane = ActivePane::Main;
                }
                (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                    app_state.active_pane = ActivePane::QueryInput;
                }

                // Navigation within panes using hjkl
                (KeyCode::Char('j'), KeyModifiers::NONE) => {
                    if app_state.active_pane == ActivePane::Databases {
                        app_state.next_database();
                        if let Some(selected) = app_state.selected_database {
                            let db_name = &app_state.databases[selected];
                            let tables = fetch_tables(db_name).await?;
                            app_state.set_tables(tables);
                        }
                    } else if app_state.active_pane == ActivePane::Tables {
                        app_state.next_table();
                    }
                }
                (KeyCode::Char('k'), KeyModifiers::NONE) => {
                    if app_state.active_pane == ActivePane::Databases {
                        app_state.previous_database();
                        if let Some(selected) = app_state.selected_database {
                            let db_name = &app_state.databases[selected];
                            let tables = fetch_tables(db_name).await?;
                            app_state.set_tables(tables);
                        }
                    } else if app_state.active_pane == ActivePane::Tables {
                        app_state.previous_table();
                    }
                }
                // Handle other input (e.g., query input)
                (KeyCode::Char(c), _) if app_state.active_pane == ActivePane::QueryInput => {
                    app_state.query.push(c);
                }
                (KeyCode::Backspace, _) if app_state.active_pane == ActivePane::QueryInput => {
                    app_state.query.pop();
                }
                (KeyCode::Enter, _) if app_state.active_pane == ActivePane::QueryInput => {
                    if let (Some(db_idx), Some(table_idx)) = (app_state.selected_database, app_state.selected_table) {
                        let db_name = &app_state.databases[db_idx];
                        let table_name = &app_state.tables[table_idx];
                        let query = app_state.query.clone();
                        let result = execute_query(db_name, table_name, &query).await;
                        app_state.query_result = match result {
                            Ok(res) => res,
                            Err(err) => vec![vec![format!("Error: {}", err)]],
                        };
                    }
                }
                (KeyCode::Char('q'), _) => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    Ok(())
}

async fn fetch_tables(db_name: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let connection_string = format!("host=localhost user=postgres password=postgres dbname={}", db_name);
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    let query = "SELECT tablename FROM pg_tables WHERE schemaname = 'public'";
    let rows = client.query(query, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}

async fn execute_query(db_name: &str, _table_name: &str, query: &str) -> Result<Vec<Vec<String>>, Box<dyn Error>> {
    let connection_string = format!("host=localhost user=postgres password=postgres dbname={}", db_name);
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    // Execute the free-form query directly
    let rows = client.query(query, &[]).await?;

    let mut results = vec![];
    for row in rows {
        let mut result_row = vec![];
        for col_idx in 0..row.len() {
            result_row.push(row.get::<usize, String>(col_idx));
        }
        results.push(result_row);
    }

    Ok(results)
}

