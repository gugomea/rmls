use std::io::{self, stdout, Stdout};
use std::rc::Rc;
use std::cell::RefCell;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent}, execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand
};
use ratatui::{prelude::*, widgets::*};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;

    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut state = State::default();
    loop {
        handle_events(&mut terminal, &mut state)?;
        if state.quit {
            break;
        }
        terminal.draw(|frame| {
            let screen = frame.size();
            frame.render_widget(Block::bordered(), screen);
            for (i, dir) in state.root.iter().iter().enumerate() {
                let dir = dir.borrow();
                let i = i as u16 + 1;
                let line_area = Rect::new(1, i, screen.width - 2, 1);
                let padding = " ".repeat(dir.id.len());
                let mut text = Text::raw(format!("{}\u{25B8} {} - {:?} - Open({})", padding, dir.name, dir.id, dir.open));
                let current = i == state.y;
                if dir.open {
                    text = Text::raw(format!("{}\u{25BC} {} - {:?} - Open({})", padding, dir.name, dir.id, dir.open));
                }
                if current {
                    text = Text::raw(format!("{}\u{25B8} {} - {:?} - Open({}).  idx: {:?}", padding, dir.name, dir.id, dir.open, state.id))
                        .fg(Color::Black)
                        .bg(Color::Gray);
                }
                frame.render_widget(text, line_area);
            }
        })?;
    }
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct State {
    y: u16,
    skip: usize,
    quit: bool,
    enter: bool,
    root: DirTree,
    id: Vec<usize>,
}


//.                        [0]           (not visible)
// ├── Cargo.lock          [0, 0]         => Node { id: [0, 0], name: "Cargo.lock", open: flase, cached_children: None }
// ├── Cargo.toml          [0, 1]
// ├── files2.sh           [0, 2]
// ├── logs.txt            [0, 3]
// ├── src                 [0, 4]         => Node { id: [0, 4], name: "Cargo.lock", open: true, cached_children: Some([..]) }
// │   ├── bin             [0, 4, 0]
// │   │   ├── recover.rs  [0, 4, 0, 1]
// │   │   ├── rm.rs       [0, 4, 0, 1]
// │   │   └── tui.rs      [0, 4, 0, 3]
// │   └── lib.rs          [0, 4, 1]
// ├── symbolic_links      [0, 5]
// │   ├── hard_link       [0, 5, 0]
// │   ├── soflink_to      [0, 5, 1]
// │   ├── soft_link       [0, 5, 2]
// │   └── source_file     [0, 5, 3]
// └── T                   [0, 6]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct Node {
    id: Vec<usize>, // there won't be two equals id, so we can derive Ord.
    name: String,
    open: bool,
    cached_children: Option<Vec<Rc<RefCell<Node>>>>, //could use jus tempty vec as no cache or no children but since there will be
                                         //more files than directires we save the extra allocation
}

impl Node {
    fn request_children(&mut self) {
        if self.cached_children.is_none() {
            self.cached_children = Some(vec![
                Rc::new(RefCell::new(Node {
                    id: [self.id.clone(), vec![0]].concat(),
                    name: "Generic Child".into(),
                    open: false,
                    cached_children: None,
                })),
                Rc::new(RefCell::new(Node {
                    id: [self.id.clone(), vec![1]].concat(),
                    name: "Generic Child".into(),
                    open: false,
                    cached_children: None,
                })),
                Rc::new(RefCell::new(Node {
                    id: [self.id.clone(), vec![2]].concat(),
                    name: "Generic Child".into(),
                    open: false,
                    cached_children: None,
                })),
            ]
            );
        }
    }

    fn iter(&self) -> Vec<Entry> {
        let mut acc = vec![];
        if let Some(ref cached_children) = self.cached_children {
            for ch in cached_children {
                acc.push(ch.clone());
                let inner = ch.borrow();
                let open = inner.open;
                if open {
                    acc.append(&mut inner.iter());
                }
            }
            return acc;
        }
        return acc;
    }
}

type Entry = Rc<RefCell<Node>>;
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct DirTree {
    root: Rc<RefCell<Node>>,
}

impl DirTree {
    fn previous(&self, mut id: Vec<usize>) -> Option<Entry> {
        match id.pop() {
            Some(last) if last > 0 => id.push(last-1),
            Some(_zero) => return self.get(&id), //has to be the root of this direcory
            None => return None,
        }

        let Some(mut current) = self.get(&id) else { 
            return None
        };

        if !current.borrow().open {
            return Some(current);
        }

        loop {
            let last = match &current.borrow().cached_children {
                Some(ch) if current.borrow().open => ch.last().unwrap().clone(), // array can't be empty
                _ => break,
            };
            current = last;
        }
        return Some(current);
    }

    fn next(&self, mut id: Vec<usize>) -> Option<Entry> {
        let Some(current) = self.get(&id) else { 
            return None
        };

        //if open, then it has at least one file
        if current.borrow().open {
            id.push(0);
            return self.get(&id);
        }

        //try to go down.
        *id.last_mut().unwrap() += 1;
        if let Some(next) = self.get(&id) {
            return Some(next);
        }
        *id.last_mut().unwrap() -= 1;

        //we have to go up, to the closes one, so we go in reverse order
        for i in (0..id.len()).rev() {
            let mut upper_id = id[..i].to_vec();
            *upper_id.last_mut().unwrap() += 1;
            if let Some(upper) = self.get(&upper_id) {
                return Some(upper);
            }
        }
        return None;
    }

    fn get(&self, id: &[usize]) -> Option<Entry> {
        let mut current_entry = None;
        let mut current_root = self.root.clone();
        for &i in &id[1..] {
            let ith_children = {
                match current_root.borrow().cached_children.as_ref() {
                    Some(children) if i < children.len() => children[i].clone(),
                    Some(_) | None => return None, // out of bounds or None
                }
            };
            current_root = ith_children.clone();
            current_entry = Some(ith_children.clone());
        }
        return current_entry;
    }

    fn iter(&self) -> Vec<Entry> {
        let mut acc = vec![]; // vec![self.root.clone()];
        if let Some(ref cached_children) = self.root.borrow().cached_children {
            for ch in cached_children {
                acc.push(ch.clone());
                let inner = ch.borrow();
                let open = inner.open;
                if open {
                    acc.append(&mut inner.iter());
                }
            }
            return acc;
        }
        return acc;
    }

}

impl Default for State {
    fn default() -> Self {
        let make_entry = |id: usize, name: String| Rc::new(RefCell::new(Node { id: vec![0, id], name, open: false, cached_children: None }));
        let children = (0..32).map(|i| make_entry(i, format!("{i}º Entry"))).collect();
        let root = DirTree {
            root: Rc::new(RefCell::new(Node {
                id: vec![0],
                name: "Root".into(),
                open: false,
                cached_children: Some(children),
            }))
        };
        
        Self {
            skip: 0,
            y: 1,
            quit: false,
            enter: false,
            root,
            id: vec![0, 0],
        }
    }
}

fn handle_events(t: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut State) -> io::Result<()> {
    let screen = t.size().unwrap();
    if event::poll(std::time::Duration::from_millis(50))? {
        match event::read()? {
            #[allow(unused_variables)]
            Event::Key(KeyEvent { code, modifiers, kind, state }) => {
                match code {
                    KeyCode::Char('j') => {
                        app.y = u16::min(screen.height - 2, app.y + 1);
                        let next = app.root.next(app.id.clone());
                        if let Some(next) = next {
                            app.id = next.borrow().id.clone();
                        }
                    }
                    KeyCode::Char('k') => {
                        app.y = u16::max(1, app.y - 1);
                        let next = app.root.previous(app.id.clone());
                        if let Some(next) = next {
                            app.id = next.borrow().id.clone();
                        }
                    }
                    KeyCode::Char('q') => app.quit = true,
                    KeyCode::Enter => {
                        app.enter = !app.enter;
                        let Some(current) = app.root.get(&app.id) else { panic!() };
                        current.borrow_mut().open ^= true; // so that assignment is not that long :)
                        current.borrow_mut().request_children();
                    }
                    _ => {} // avoiding rest of characters
                }
            }
            Event::Mouse(_) => {}
            _ => {} // avoiding rest of events...
        }
    }
    Ok(())
}
