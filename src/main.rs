use ratatui::{
    backend::{CrosstermBackend},
    widgets::{Block, Borders, List, ListItem, ListState},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color},
    Terminal,
};
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
use tokio_postgres::NoTls;
use std::{error::Error, io};

struct AppState {
    databases: Vec<String>,
    selected_database: Option<usize>,
    tables: Vec<String>,
}

impl AppState {
    fn new(databases: Vec<String>) -> Self {
        Self {
            databases,
            selected_database: Some(0), // Start by selecting the first database
            tables: vec![],
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
    // Connect to PostgreSQL to fetch databases
    let (client, connection) = tokio_postgres::connect("host=localhost user=postgres password=postgres", NoTls).await?;
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

            // Create a layout with a sidebar and main area
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(size);

            // Create a vertical layout for the sidebar
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(horizontal_chunks[0]);

            // Sidebar for the list of databases
            let db_items: Vec<ListItem> = app_state
                .databases
                .iter()
                .map(|db| ListItem::new(db.clone()))
                .collect();

            let mut db_list_state = ListState::default();
            db_list_state.select(app_state.selected_database);

            let db_sidebar = List::new(db_items)
                .block(Block::default().title("Databases").borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Yellow).fg(Color::Black))
                .highlight_symbol("> ");

            // Sidebar for the list of tables
            let table_items: Vec<ListItem> = app_state
                .tables
                .iter()
                .map(|table| ListItem::new(table.clone()))
                .collect();

            let table_sidebar = List::new(table_items)
                .block(Block::default().title("Tables").borders(Borders::ALL));

            // Main content area
            let main_area = Block::default()
                .title("Postgres GUI")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White).bg(Color::Blue));

            // Render widgets
            f.render_stateful_widget(db_sidebar, vertical_chunks[0], &mut db_list_state);
            f.render_widget(table_sidebar, vertical_chunks[1]);
            f.render_widget(main_area, horizontal_chunks[1]);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('j') => {
                    app_state.next_database();
                    if let Some(selected) = app_state.selected_database {
                        let db_name = &app_state.databases[selected];
                        let tables = fetch_tables(db_name).await?;
                        app_state.set_tables(tables);
                    }
                }
                KeyCode::Char('k') => {
                    app_state.previous_database();
                    if let Some(selected) = app_state.selected_database {
                        let db_name = &app_state.databases[selected];
                        let tables = fetch_tables(db_name).await?;
                        app_state.set_tables(tables);
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
    // Connect to the selected database
    let connection_string = format!("host=localhost user=postgres password=postgres dbname={}", db_name);
    let (client, connection) = tokio_postgres::connect(&connection_string, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Connection error: {}", e);
        }
    });

    // Query for tables
    let query = "SELECT tablename FROM pg_tables WHERE schemaname = 'public'";
    let rows = client.query(query, &[]).await?;
    Ok(rows.iter().map(|row| row.get(0)).collect())
}
