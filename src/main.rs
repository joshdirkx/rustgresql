use ratatui::{
    backend::{CrosstermBackend},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color},
    Terminal,
};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use tokio_postgres::NoTls;
use std::{error::Error, io};

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
    query: String, // Store the current query input
    query_result: String, // Store the result of the executed query
    active_pane: ActivePane,
}

impl AppState {
    fn new(databases: Vec<String>) -> Self {
        Self {
            databases,
            selected_database: Some(0),
            tables: vec![],
            query: String::new(),
            query_result: String::new(),
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

    fn set_tables(&mut self, tables: Vec<String>) {
        self.tables = tables;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Connect to PostgreSQL
    let (client, connection) = tokio_postgres::connect("host=localhost user=postgres password=secret", NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    // Fetch the list of databases
    let rows = client.query("SELECT datname FROM pg_database WHERE datistemplate = false", &[]).await?;
    let databases: Vec<String> = rows.iter().map(|row| row.get(0)).collect();

    // Initialize application state
    let mut app_state = AppState::new(databases);

    // Fetch initial tables for the first database
    if let Some(selected) = app_state.selected_database {
        let db_name = &app_state.databases[selected];
        let tables = fetch_tables(db_name).await?;
        app_state.set_tables(tables);
    }

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(|f| {
            let size = f.size();

            // Create a layout with three main sections
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(size);

            // Left vertical layout for databases and tables
            let vertical_chunks_left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(horizontal_chunks[0]);

            // Right vertical layout for main area and query input
            let vertical_chunks_right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(80), // Main area
                    Constraint::Percentage(20), // Query input
                ])
                .split(horizontal_chunks[1]);

            // Sidebar for databases
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

            // Sidebar for tables
            let table_items: Vec<ListItem> = app_state
                .tables
                .iter()
                .map(|table| ListItem::new(table.clone()))
                .collect();

            let table_sidebar = List::new(table_items)
                .block(Block::default()
                    .title("Tables")
                    .borders(Borders::ALL)
                    .style(if app_state.active_pane == ActivePane::Tables {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }));

            // Main content area
            let main_area = Block::default()
                .title("Main Area")
                .borders(Borders::ALL)
                .style(if app_state.active_pane == ActivePane::Main {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });

            // Query input area
            let query_input = Paragraph::new(app_state.query.clone())
                .block(Block::default()
                    .title("Enter Query")
                    .borders(Borders::ALL)
                    .style(if app_state.active_pane == ActivePane::QueryInput {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }));

            // Render widgets
            f.render_stateful_widget(db_sidebar, vertical_chunks_left[0], &mut db_list_state);
            f.render_widget(table_sidebar, vertical_chunks_left[1]);
            f.render_widget(main_area, vertical_chunks_right[0]);
            f.render_widget(query_input, vertical_chunks_right[1]);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
match (key.code, key.modifiers) {
    // Switch panes with Ctrl + hjkl
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
    // Quit
    (KeyCode::Char('q'), _) => break,
    // Handle query input (only in QueryInput pane)
    (KeyCode::Char(c), _) if app_state.active_pane == ActivePane::QueryInput => {
        app_state.query.push(c);
    }
    (KeyCode::Backspace, _) if app_state.active_pane == ActivePane::QueryInput => {
        app_state.query.pop();
    }
    (KeyCode::Enter, _) if app_state.active_pane == ActivePane::QueryInput => {
        if let Some(selected) = app_state.selected_database {
            let db_name = &app_state.databases[selected];
            let result = execute_query(db_name, &app_state.query).await;
            app_state.query_result = match result {
                Ok(res) => res,
                Err(err) => format!("Error: {}", err),
            };
        }
    }
    _ => {}
}
        }
    }

    disable_raw_mode()?;
    Ok(())
}

async fn fetch_tables(db_name: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let connection_string = format!("host=localhost user=postgres password=secret dbname={}", db_name);
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

async fn execute_query(db_name: &str, query: &str) -> Result<String, Box<dyn Error>> {
    let connection_string = format!("host=localhost user=postgres password=secret dbname={}", db_name);
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    let rows = client.query(query, &[]).await?;
    Ok(format!("Executed query successfully. Rows returned: {}", rows.len()))
}
