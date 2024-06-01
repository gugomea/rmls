use std::env::args;
use std::ffi::OsString;
use std::fs::metadata;
use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::cell::RefCell;
use crossterm::event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent}, execute, terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}, ExecutableCommand
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Paragraph};
use std::panic::{set_hook, take_hook};

pub fn init_panic_hook() {
    let _original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        disable_raw_mode().unwrap();
        //stdout().execute(LeaveAlternateScreen).unwrap();
        println!("{}", panic_info.to_string());
    }));
}

fn main() -> io::Result<()> {
    init_panic_hook();
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), 
        Clear(ClearType::All),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let dir_name = args().nth(1).unwrap();
    let mut state = State::new(dir_name);
    let mut current_height: u16 = terminal.size().unwrap().height - 2;
    loop {
        handle_events(&mut state, current_height)?;
        if state.quit {
            break;
        }
        let entries: Vec<Entry> = DirEntry::entries(&state.root, false);
        current_height = u16::min(entries.len() as u16, terminal.size().unwrap().height - 2);
        terminal.draw(|frame| {
            let screen = frame.size();
            //frame.render_widget(Block::bordered(), screen);
            let info = format!("Entries: {}\nCurrent Height: {}\nY: {}\nOffset: {}\nCurrent:{:?}\nDeleted:{:?}", 
                entries.len(),
                current_height,
                state.y,
                state.skip,
                state.id,
                state.deletions);
            frame.render_widget(Paragraph::new(info).block(Block::bordered().border_type(BorderType::Rounded)), Rect { x: screen.width / 2, y: screen.height / 2, width: screen.width / 2, height: screen.height / 2 });
            for (i, dir) in entries.iter().skip(state.skip).take(current_height as usize).enumerate() {
                let dir = dir.borrow();
                let i = i as u16 + 1;
                let line_area = Rect::new(1, i, screen.width - 2, 1);
                let padding = " ".repeat(dir.id.len());
                let arrow = match (dir.is_file, dir.open) {
                    (false, true) => "\u{2B9F}",
                    (false, false) => "\u{2B9E}",
                    _ => " ",
                };
                let name = dir.name.to_str().unwrap_or("Non UTF-8 name");
                let mut text = Text::raw(format!("{padding}{arrow} {name}"));
                let current = i == u16::min(state.y as u16, screen.height-2);
                if dir.deleted {
                    text = Text::raw(format!("{padding}{arrow} {name}"))
                        .fg(Color::Black)
                        .bg(Color::Yellow);
                }
                if current {
                    text = Text::raw(format!("{padding}{arrow} {name}"))
                        .fg(Color::Black)
                        .bg(Color::Gray);
                }
                frame.render_widget(text, line_area);
            }
        })?;
    }
    disable_raw_mode()?;
    execute!(stdout(), PopKeyboardEnhancementFlags)?;
    stdout().execute(LeaveAlternateScreen)?;
    println!("Selected entries:");
    DirEntry::entries(&state.root, true)
        .iter().filter(|x| x.borrow().deleted)
        .for_each(|entry| println!("{:?}", Path::new(&entry.borrow().path)
                                                .join(&entry.borrow().name)));
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct State {
    y: u16,
    skip: usize,
    quit: bool,
    enter: bool,
    root: Entry,
    id: Vec<usize>,
    deletions: Vec<Vec<usize>>,
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
type Entry = Rc<RefCell<Node>>;
#[derive(Debug, Clone, Default)]
struct Node {
    id: Vec<usize>, // there won't be two equals id, so we can derive Ord.
    name: OsString,
    open: bool,
    path: PathBuf,
    is_file: bool,
    deleted: bool,
    parent: Option<Weak<RefCell<Node>>>,
    cached_children: Option<Vec<Entry>>, //could use jus tempty vec as no cache or no children but since there will be
                                         //more files than directires we save the extra allocation
}

trait DirEntry {
    fn insert_node(root: &Self, child: Self);
    fn previous(root: &Self) -> Option<Entry>;
    fn next(root: &Self) -> Option<Entry>;
    fn request_children(root: &Self);
    fn entries(root: &Self, all_nodes: bool) -> Vec<Entry>;
    fn get(root: &Self, id: &[usize]) -> Option<Entry>;
    fn detach(root: &Self);
    fn attach(root: &Self);
}

impl DirEntry for Entry {
    fn detach(root: &Self) {
        root.borrow_mut().deleted = true;
        if root.borrow().is_file {
            return;
        }
        let borrow = root.borrow_mut();
        let Some(children) = borrow.cached_children.as_ref() else {
            return;
        };
        for ch in children {
            DirEntry::detach(ch);
        }
    }
    fn attach(root: &Self) {
        root.borrow_mut().deleted = false;
        if root.borrow().is_file {
            return;
        }
        let borrow = root.borrow_mut();
        let Some(children) = borrow.cached_children.as_ref() else {
            return;
        };
        for ch in children {
            DirEntry::attach(ch);
        }
    }
    fn get(root: &Self, id: &[usize]) -> Option<Entry> {
        let mut root = root.clone();
        for &i in &id[1..] {
            let ith_child = match root.borrow().cached_children.as_ref() {
                Some(ch) => ch[i].clone(),
                None => {
                    //panic!("i: {}, id: {:?}\nroot: {:?}", i, id, root);
                    return None;
                }
            };
            root = ith_child.clone();
        }
        return Some(root);
    }
    fn entries(root: &Self, all_nodes: bool) -> Vec<Entry> {
        let mut acc = vec![];//not show root.
        if let Some(ref children) = root.borrow().cached_children {
            for ch in children {
                let open = ch.borrow().open;
                let visible = !ch.borrow().deleted;
                if visible || all_nodes {
                    acc.push(ch.clone());
                }
                if all_nodes || (open && visible) {
                    acc.append(&mut DirEntry::entries(ch, all_nodes));
                }
            }
        }
        return acc;
    }

    fn insert_node(root: &Entry, child: Self) {
        let mut root_mut = root.borrow_mut();
        let children = root_mut.cached_children.get_or_insert(vec![]);
        children.push(child.clone());
        child.borrow_mut().parent = Some(Rc::downgrade(&root));
    }

    fn next(root: &Entry) -> Option<Entry> {
        //try to go to the first 
        let mut skip = 0;
        let mut parent = root.clone();
        loop {
            {
                let parent_ = parent.borrow();
                let children = parent_.cached_children.as_ref();
                if let Some(children) = children {
                    let child = children.iter()
                        .skip(skip)
                        .find(|x| !x.borrow().deleted)
                        .cloned();
                    if child.is_some() && parent_.open {
                        //println!("\n\nnext: {:?}", child.as_ref().unwrap().borrow().id);
                        return child;
                    }
                }
            }
            {
                let id = &parent.borrow().id;
                match id.last() {
                    Some(n) => skip = *n + 1,
                    None => break,
                }
            }
            parent = {
                let tmp_parent = parent.borrow();
                match tmp_parent.parent.as_ref() {
                    Some(p) => p.upgrade().unwrap(),
                    None => break,
                }
            };
        }
        return None;
    }

    fn previous(root: &Entry) -> Option<Entry> {
        //go to the closest on the left, once there go full down right
        //try to go to the first 
        let id = *root.borrow().id.last().unwrap();
        let mut take = id;
        let Some(parent) = root.borrow().parent .clone() else { 
            return None
        };
        let mut parent = parent.upgrade().unwrap();
        let mut new_root = loop {
            {
                let parent_ = parent.borrow();
                let children = parent_.cached_children.as_ref();
                if let Some(children) = children {
                    let child = children.iter()
                        .take(take).rev()
                        .find(|x| !x.borrow().deleted)
                        .cloned();
                    //we found a child to the left of ourselves.
                    if child.is_some() {
                        //println!("PREVIOUS: {:?}", child);
                        break child.unwrap();
                    }
                    //we didn't but if the parent is valid, we return it
                    if !parent_.deleted && parent_.id != &[0] {
                        return Some(parent.clone());
                    }
                }
            }
            {
                let id = &parent.borrow().id;
                match id.last() {
                    Some(n) => take = *n,
                    None => return None,
                }
            }
            parent = {
                let tmp_parent = parent.borrow();
                match tmp_parent.parent.as_ref() {
                    Some(p) => p.upgrade().unwrap(),
                    None => return None,
                }
            };
        };

        //go full right
        loop {
            new_root = {
                let borrow = new_root.borrow();
                if !borrow.open {
                    break;
                }
                match borrow.cached_children.as_ref() {
                    Some(children) => {
                        match children.iter().rev().find(|x| !x.borrow().deleted) {
                            Some(value) => value.clone(),
                            None => break,
                        }
                    }
                    None => break,
                }
            };
        }
        return Some(new_root);
    }

    fn request_children(root: &Self){
        if root.borrow().cached_children.is_some() {
            return;
        }
        let mut children = vec![];
        let new_path = Path::new(&root.borrow().path).join(&root.borrow().name);
        let entries = std::fs::read_dir(&new_path)
            .expect("Error opening dir")
            .into_iter();
        for (i, entry) in entries.enumerate() {
            let path = entry.unwrap().path();
            let is_file = path.is_file();
            let name = path.file_name()
                .expect("Invalid OSstr")
                .to_owned();
            let id = [root.borrow().id.clone(), vec![i]].concat();
            let node = Rc::new(RefCell::new(Node {
                id, name, deleted: false,
                open: false, is_file,
                cached_children: None,
                parent: None,
                path: new_path.clone()
            }));
            DirEntry::insert_node(root, node.clone());
            children.push(node);
        }
        root.borrow_mut().cached_children = Some(children);
    }
}


impl State {
    fn new(name: String) -> Self {
        let name = name.into();
        let _mt = metadata(&name).expect("File doen't exist");
        let root = Rc::new(RefCell::new(Node {
            id: vec![0],
            name, open: true,
            is_file: false,
            deleted: false,
            cached_children: None,
            parent: None,
            path: "".into(),
        }));
        DirEntry::request_children(&root);
        Self {
            skip: 0, y: 1,
            quit: false,
            enter: false,
            root, id: vec![0, 0],
            deletions: vec![],
        }
    }
}

fn go_up(app: &mut State) {
    //we are at the top, but we have skipped(at least one), so we show the skipped at the top
    if app.y == 1 && app.skip > 0{
        app.skip -= 1;
    } else if app.y != 1 { // just go up.
        app.y -= 1;
    }
    let Some(current) = DirEntry::get(&app.root, &app.id) else { panic!("ERROR GETTING ID: {:?}", app.id) };
    let next = DirEntry::previous(&current);
    if let Some(next) = next {
        app.id = next.borrow().id.clone();
    }
}

fn go_down(app: &mut State, height: u16) {
    let Some(current) = DirEntry::get(&app.root, &app.id) else { panic!("ERROR GETTING ID: {:?}", app.id) };
    let next = DirEntry::next(&current);
    if let Some(next) = next {
        let next = next.borrow().id.clone();
        app.id = next;
        match app.y < height {
            true => app.y += 1,     // we are inside the window, just go down.
            false => app.skip += 1, // we are not, so we skip the upper files(to show the one behind us)
        }
    }
}

fn handle_events(app: &mut State, height: u16) -> io::Result<()> {
    if event::poll(std::time::Duration::from_millis(50))? {
        match event::read()? {
            #[allow(unused_variables)]
            Event::Key(KeyEvent { code, modifiers, kind, state }) => {
                match code {
                    KeyCode::Char('j') => go_down(app, height),
                    KeyCode::Char('k') => go_up(app),
                    KeyCode::Char('q') => app.quit = true,
                    KeyCode::Char('u') => {
                        if let Some(deleted) = app.deletions.pop() {
                            let deleted = DirEntry::get(&app.root, &deleted).unwrap();
                            DirEntry::attach(&deleted);
                            ////check if the file goes before of after us, to move up or down
                            //let file_id = &file.borrow().id;
                            //if file_id < &app.id {
                            //    match app.y < height {
                            //        true => app.y += 1,     // we are inside the window, just go down.
                            //        false => app.skip += 1, // we are not, so we skip the upper files(to show the one behind us)
                            //    }
                            //} else {
                            //    go_up(app);
                            //}
                        } else {
                            //print that it is last deletion at bottom or pop up!
                        }
                    }
                    KeyCode::Char('d') => {
                        let Some(current) = DirEntry::get(&app.root, &app.id) else {
                            panic!("ERROR GETTING ID: {:?}", app.id)
                        };
                         // CHECK FOR REPEATED
                        if current.borrow().deleted {
                            panic!("DELETED");
                            //return Ok(());
                        }
                        app.deletions.push(current.borrow().id.clone());
                        DirEntry::detach(&current);
                        if let Some(next) = DirEntry::next(&current) {
                            app.id = next.borrow().id.clone();
                        } else if let Some(prev) = DirEntry::previous(&current) {
                            app.id = prev.borrow().id.clone();
                        } else {
                            panic!("ALL FILES DELETED");
                        }
                    }
                    KeyCode::Enter => {
                        app.enter = !app.enter;
                        let Some(current) = DirEntry::get(&app.root, &app.id) else { panic!("root: {:?}\nID: {:?}", app.root, app.id) };
                        if !current.borrow().is_file {
                            current.borrow_mut().open ^= true; // so that assignment is not that long :)
                            DirEntry::request_children(&current);
                        }
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
