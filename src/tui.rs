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
use std::panic::{set_hook, take_hook};

pub fn init_panic_hook() {
    let _original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        disable_raw_mode().unwrap();
        stdout().execute(LeaveAlternateScreen).unwrap();
        println!("{}", panic_info.to_string());
    }));
}

pub fn tui(dir_name: String) -> io::Result<Vec<PathBuf>> {
    init_panic_hook();
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    execute!(stdout(), 
        Clear(ClearType::All),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut state = State::new(dir_name);
    let mut current_height: u16 = terminal.size().unwrap().height - 2;
    loop {
        handle_events(&mut state, current_height)?;
        if state.quit {
            break;
        }
        let entries: Vec<Entry> = DirEntry::entries(&state.root, EntryState::Visible);
        terminal.draw(|frame| {
            let screen = frame.size();
            current_height = u16::min(entries.len() as u16, screen.height - 2);
            //frame.render_widget(Block::bordered(), screen);
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
                if current {
                    text = Text::raw(format!("{padding}{arrow} {name}"))
                        .fg(Color::Black)
                        .bg(Color::Gray);
                }
                if let Some((ref id_selected, _, _)) = state.selecting {
                    let id = &state.id;
                    //selected - current - state.id   || state.id - current - selected
                    if (id_selected <= &dir.id && &dir.id <= id) || (id_selected >= &dir.id && &dir.id >= id) {
                        text = Text::raw(format!("{padding}{arrow} {name}"))
                            .fg(Color::Black)
                            .bg(Color::Yellow);
                    }
                    
                }
                frame.render_widget(text, line_area);
            }
        })?;
    }

    disable_raw_mode()?;
    execute!(stdout(), PopKeyboardEnhancementFlags)?;
    stdout().execute(LeaveAlternateScreen)?;

    println!("Selected entries:");
    let path = |x: Entry| Path::new(&x.borrow().path).join(&x.borrow().name);
    Ok(
        DirEntry::entries(&state.root, EntryState::Deleted)
        .into_iter()
        .map(path)
        .collect()
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum EntryState {
    Visible,
    Deleted,
    All,
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
    deletions: Vec<(Vec<Vec<usize>>, u16, usize)>,//[ ([id1, id2...],y,skip) ... ]
    selecting: Option<(Vec<usize>, u16, usize)>,
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
    fn entries(root: &Self, all_nodes: EntryState) -> Vec<Entry>;
    fn get(root: &Self, id: &[usize]) -> Option<Entry>;
    fn go(root: &Self, id: &[usize]) -> Option<()>;//just return option so I can do '?', could have returned boolean
    fn detach(root: &Self);
    fn attach(root: &Self);
}

impl DirEntry for Entry {
    //go to certain id. Open directories if needed.
    //If find deleted directory, then return None;
    fn go(root: &Self, id: &[usize]) -> Option<()> {
        let mut current = root.clone();
        for &i in &id[1..id.len()-1] {
            let curr = current
                .borrow().cached_children
                .as_ref()?[i].clone();
            current = curr;
            current.borrow_mut().open = true;
            if current.borrow().deleted {
                return None;
            }
        }
        return Some(());
    }
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
    fn entries(root: &Self, visibiliy: EntryState) -> Vec<Entry> {
        let mut acc = vec![];//not show root.
        if let Some(ref children) = root.borrow().cached_children {
            for ch in children {
                let borrow = ch.borrow();
                let open = borrow.open;
                let del = borrow.deleted;
                match (visibiliy, del) {
                    (EntryState::Deleted, true) => acc.push(ch.clone()),
                    (EntryState::Deleted, false) => acc.append(&mut DirEntry::entries(ch, visibiliy)),
                    (EntryState::All, _) => {
                        acc.push(ch.clone());
                        acc.append(&mut DirEntry::entries(ch, visibiliy));
                    }
                    (EntryState::Visible, false) => {
                        acc.push(ch.clone());
                        if open {
                            acc.append(&mut DirEntry::entries(ch, visibiliy));
                        }
                    }
                    (EntryState::Visible, true) => (), //avoid this file, is deleted
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
        //scan the children of parent from left to right
        let mut skip = 0;
        //our parent is the current node, so we skip 0 childs (since we skip the ones on our left, the first one skips 0)
        let mut parent = root.clone();
        loop {
            parent = {
                let parent_ = parent.borrow();
                let children = parent_.cached_children.as_ref();
                if let Some(children) = children {
                    //scan children from left to right, skipping the siblings before us
                    let child = children.iter()
                        .skip(skip)
                        .find(|x| !x.borrow().deleted)
                        .cloned();
                    //we could only reach the child if the parent was open
                    if child.is_some() && parent_.open {
                        return child;
                    }
                }
                let id = &parent_.id;
                // the index, represents the position of the child, so + 1 since we want to start to our right
                skip = id.last()? + 1;
                //go up one level, and try again
                parent_.parent.as_ref()?//if no parent, we can't keep searching
                    .upgrade().unwrap()
            };
        }
    }

    fn previous(root: &Entry) -> Option<Entry> {
        //go to the closest on the left, once there go full down right
        //try to go to the first 
        let mut take = *root.borrow().id.last()?;
        let parent = root.borrow().parent.clone()?;
        let mut parent = parent.upgrade().unwrap();
        let mut new_root = loop {
            parent = {
                let parent_ = parent.borrow();
                let child = parent_.cached_children.as_ref()?
                    .iter()
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
                take = *parent_.id.last()?;
                parent_.parent.as_ref()?
                    .upgrade().unwrap()
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
            selecting: None,
        }
    }
}

fn go_up(app: &mut State) {
    let Some(current) = DirEntry::get(&app.root, &app.id) else { panic!("ERROR GETTING ID: {:?}", app.id) };
    let next = DirEntry::previous(&current);
    if let Some(next) = next {
        if next.borrow().open && app.selecting.is_some() {
            return;
        }
        //we are at the top, but we have skipped(at least one), so we show the skipped at the top
        if app.y == 1 && app.skip > 0{
            app.skip -= 1;
        } else if app.y != 1 { // just go up.
            app.y -= 1;
        }
        app.id = next.borrow().id.clone();
    }
}

fn go_down(app: &mut State, height: u16) {
    let Some(current) = DirEntry::get(&app.root, &app.id) else { panic!("ERROR GETTING ID: {:?}", app.id) };

    let next = DirEntry::next(&current);
    if let Some(next) = next {
        if next.borrow().open && app.selecting.is_some() {
            return;
        }
        let next = next.borrow().id.clone();
        app.id = next;
        match app.y < height {
            true => app.y += 1,     // we are inside the window, just go down.
            false => app.skip += 1, // we are not, so we skip the upper files(to show the one behind us)
        }
    }
}

fn delete(app: &mut State, from: &[usize], to: &[usize]) {
    let mut id = from;
    let (mut acc, y, skip) = (vec![], app.y, app.skip);
    loop {
            let Some(current) = DirEntry::get(&app.root, id) else {
                panic!("ERROR GETTING ID: {:?}", app.id)
            };
            DirEntry::detach(&current);
            acc.push(current.borrow().id.clone());
            if let Some(next) = DirEntry::next(&current) {
                app.id = next.borrow().id.clone();
            } else if let Some(_prev) = DirEntry::previous(&current) {
                go_up(app);// we have to move.
            } else {
                panic!("ALL FILES DELETED");
            }
            if &current.borrow().id == &to {
                app.deletions.push((acc, y, skip));
                break;
            }
            id = &app.id;
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
                        //If you delete files and then directory, when restoring you will have the
                        //full directory.Maybe keep poping while the id's are already attached.

                        if let Some((deletions, y, skip)) = app.deletions.pop() {
                            app.id = deletions.first().unwrap().clone();
                            app.skip = skip;
                            app.y = y;
                            for deleted in deletions {
                                let del= DirEntry::get(&app.root, &deleted).unwrap();//We know its in the tree
                                DirEntry::attach(&del);
                                if DirEntry::go(&app.root, &deleted).is_none() {//impossible
                                    unreachable!("FOUND DELETED DIRECTORY WHILE TRAVERSING");
                                }
                            }
                        } else {
                            //print that it is last deletion at bottom or pop up!
                        }
                    }
                    KeyCode::Char('d') => {
                        let id = &app.id.clone();
                        match app.selecting.take() {
                            Some((ref s, y, skip)) => {
                                if s < &app.id {
                                    app.skip = skip;
                                    app.y = y;
                                    delete(app, s, id);
                                } else {
                                    delete(app, id, s);
                                }
                            }
                            None => delete(app, id, id),
                        }
                        //panic!("CURRENT: {:?}\nHEIGHT: {}\nY: {}", app.id, height, app.y);
                    }
                    KeyCode::Char('V') => {
                        let Some(current) = DirEntry::get(&app.root, &app.id) else {
                            panic!("ERROR GETTING ID: {:?}", app.id)
                        };
                        let borrow = current.borrow();
                        if borrow.open {
                            panic!("TODO. POP UP: CLOSE DIRECTORY(SPACE)\n{:?}", borrow.id);
                        } else {
                            app.selecting = Some((current.borrow().id.clone(), app.y, app.skip));
                        }
                    }
                    KeyCode::Esc => app.selecting = None,
                    KeyCode::Char(' ') => {
                        if app.selecting.is_none() {
                            app.enter = !app.enter;
                            let Some(current) = DirEntry::get(&app.root, &app.id) else { panic!("root: {:?}\nID: {:?}", app.root, app.id) };
                            if !current.borrow().is_file {
                                current.borrow_mut().open ^= true; // so that assignment is not that long :)
                                DirEntry::request_children(&current);
                            }
                        }
                    }
                    _ => {} // avoiding rest of characters
                }
            }
            Event::Mouse(_) => {}
            _ => {} // avoiding rest of events...
        }
        //while the cursor is not on place keep going up.
        while app.y > height {
            go_up(app);
        }
    }
    Ok(())
}
