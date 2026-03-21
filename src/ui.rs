use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::{AddrValidity, App, check_address};

pub fn draw(f: &mut Frame<'_>, app: &App, version_label: &str) {
    let size = f.size();

    let bg_block = Block::default().style(Style::default());
    f.render_widget(bg_block, size);

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4)])
        .split(size);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(root[0]);

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(0),
            Constraint::Length(6),
        ])
        .split(main_chunks[0]);

    let node_info_paragraph = Paragraph::new(app.node_info.as_str())
        .block(
            Block::default()
                .title("Node Info")
                .borders(Borders::ALL)
                .border_style(Style::default()),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(node_info_paragraph, left_chunks[0]);

    let wallet_info_paragraph = Paragraph::new(app.wallet_info_text())
        .block(Block::default().title("Wallet Info").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(wallet_info_paragraph, left_chunks[1]);

    let items: Vec<ListItem> = app
        .commands
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let mut item = ListItem::new(cmd.to_string());
            if i == app.selected {
                item = item.style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
            item
        })
        .collect();

    let list = List::new(items).block(Block::default().title("Commands").borders(Borders::ALL));
    f.render_widget(list, left_chunks[2]);

    let price_paragraph = Paragraph::new(app.price_info.as_str())
        .block(
            Block::default()
                .title("BTC Prices")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(price_paragraph, left_chunks[3]);

    let height = main_chunks[1].height as usize;
    let visible_height = height.saturating_sub(2);
    let visible_lines = app.visible_output_lines(visible_height);
    let output_text = visible_lines.join("\n");

    let paragraph = Paragraph::new(output_text)
        .block(Block::default().title("Output").borders(Borders::ALL))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, main_chunks[1]);

    draw_help(f, root[1], app);

    if app.show_qr_overlay {
        draw_overlay(f, size, app);
    }

    draw_version_label(f, size, version_label);
}

fn draw_help(f: &mut Frame<'_>, area: Rect, app: &App) {
    let orange = Color::Rgb(255, 165, 0);
    let help_lines: Vec<Line> = if app.show_qr_overlay {
        vec![
            Line::from(Span::styled(
                "Overlay keys:",
                Style::default().fg(orange).add_modifier(Modifier::BOLD),
            )),
            Line::from(
                "Ctrl+N=new(save)  Ctrl+G=getnew  Ctrl+C=copy  ↑/↓=select saved  ←/→ Home End Backspace Delete=edit  Ctrl+X=close",
            ),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "Main keys:",
                Style::default().fg(orange).add_modifier(Modifier::BOLD),
            )),
            Line::from(
                "↑/↓=select command  Enter=run  r=refresh  j/k=scroll output  h=hide/show amounts w=QR overlay  q=quit",
            ),
        ]
    };

    let help = Paragraph::new(help_lines).wrap(Wrap { trim: true }).block(
        Block::default()
            .title("Help")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(orange)),
    );
    f.render_widget(help, area);
}

fn draw_overlay(f: &mut Frame<'_>, size: Rect, app: &App) {
    let orange = Color::Rgb(245, 200, 66);
    let area = centered_rect(80, 75, size);
    f.render_widget(Clear, area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(orange))
        .title(" Address Book & QR (edit left • list right) ");
    f.render_widget(outer, area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8)])
        .split(cols[0]);

    let validity = check_address(&app.address);
    let (input_title, input_style, qr_title, qr_style, qr_dim) = match validity {
        AddrValidity::Empty => (
            " BTC Address ".to_string(),
            Style::default().fg(Color::Yellow),
            " Bitcoin QR Code — (enter an address) ".to_string(),
            Style::default().fg(Color::Yellow),
            true,
        ),
        AddrValidity::Invalid => (
            " BTC Address — INVALID ".to_string(),
            Style::default().fg(Color::Red),
            " Bitcoin QR Code — INVALID ".to_string(),
            Style::default().fg(Color::Red),
            true,
        ),
        AddrValidity::ValidAny(net) => {
            let label = match net {
                bitcoin::Network::Bitcoin => "VALID (mainnet)",
                bitcoin::Network::Testnet => "VALID (testnet)",
                bitcoin::Network::Testnet4 => "VALID (testnet4)",
                bitcoin::Network::Signet => "VALID (signet)",
                bitcoin::Network::Regtest => "VALID (regtest)",
            };
            (
                " BTC Address — VALID ".to_string(),
                Style::default().fg(Color::Green),
                format!(" Bitcoin QR Code — {label} "),
                Style::default().fg(Color::Green),
                false,
            )
        }
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(orange))
        .title(Span::styled(input_title, input_style));
    let input = Paragraph::new(app.address.clone()).block(input_block);
    f.render_widget(input, left[0]);

    let cursor_x = (left[0].x + 1).saturating_add(app.addr_cursor as u16);
    let cursor_y = left[0].y + 1;
    f.set_cursor(
        cursor_x.min(left[0].x + left[0].width.saturating_sub(2)),
        cursor_y,
    );

    let qr_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(orange))
        .title(Span::styled(qr_title, qr_style));
    let qr_text = if qr_dim {
        String::new()
    } else {
        generate_qr_unicode(&app.address)
    };
    let mut qr_par = Paragraph::new(qr_text).block(qr_block);
    if qr_dim {
        qr_par = qr_par.style(Style::default().fg(Color::DarkGray));
    }
    f.render_widget(qr_par, left[1]);

    let list_items: Vec<ListItem> = app
        .addr_book
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let date_str = entry.created_at.format("%Y-%m-%d %H:%M").to_string();
            let shown = if entry.address.len() > 22 {
                format!(
                    "{}  {}…{}",
                    date_str,
                    &entry.address[..12],
                    &entry.address[entry.address.len() - 8..]
                )
            } else {
                format!("{}  {}", date_str, entry.address)
            };
            let mut item = ListItem::new(shown);
            if i == app.addr_selected {
                item = item.style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
            item
        })
        .collect();

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(orange))
        .title(" Addresses (↑/↓ select) ");
    let list = List::new(list_items).block(list_block);
    f.render_widget(list, cols[1]);
}

fn draw_version_label(f: &mut Frame<'_>, size: Rect, version_label: &str) {
    let version_area = Rect {
        x: size.x,
        y: size.y,
        width: size.width,
        height: 1,
    };

    let version_text = Paragraph::new(Line::from(Span::styled(
        version_label,
        Style::default()
            .fg(Color::Rgb(180, 180, 180))
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Right);

    f.render_widget(version_text, version_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn generate_qr_unicode(data: &str) -> String {
    let safe = if data.is_empty() { " " } else { data };
    match qrcode::QrCode::new(safe) {
        Ok(code) => code
            .render::<qrcode::render::unicode::Dense1x2>()
            .quiet_zone(false)
            .build(),
        Err(_) => String::from("Unable to render QR"),
    }
}
