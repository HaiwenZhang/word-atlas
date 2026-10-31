//! Typed row mapping and minimal updates for one logical progress edit.

use std::collections::BTreeMap;

use atlas_domain::{
    Card, Progress, Settings,
    model::{DayStat, Session, TypingRound},
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Error, Snapshot};

pub(super) fn revision(connection: &Connection) -> Result<i64, Error> {
    Ok(connection
        .query_row("SELECT revision FROM profile WHERE id=1", [], |row| {
            row.get(0)
        })
        .optional()?
        .unwrap_or(0))
}

pub(super) fn read_snapshot(connection: &Connection) -> Result<Option<Snapshot>, Error> {
    let profile: Option<(String, u8, i64)> = connection
        .query_row(
            "SELECT dataset_id,progress_version,revision FROM profile WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((dataset, version, revision)) = profile else {
        return Ok(None);
    };
    let mut progress = Progress::new(&dataset);
    progress.version = version;
    let mut cards=connection.prepare("SELECT word_id,due,stability,difficulty,elapsed_days,scheduled_days,learning_steps,reps,lapses,state,last_review,seen FROM cards ORDER BY word_id")?;
    let records = cards.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            Card {
                due: row.get(1)?,
                s: row.get(2)?,
                d: row.get(3)?,
                el: row.get(4)?,
                sd: row.get(5)?,
                ls: row.get(6)?,
                reps: row.get(7)?,
                lapses: row.get(8)?,
                st: row.get(9)?,
                last: row.get(10)?,
                seen: row.get(11)?,
            },
        ))
    })?;
    for record in records {
        let (id, card) = record?;
        progress.cards.insert(id, card);
    }
    let mut days = connection.prepare(
        "SELECT day,new_words,reviews,correct,active_seconds FROM day_stats ORDER BY day",
    )?;
    let records = days.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            DayStat {
                n: r.get(1)?,
                r: r.get(2)?,
                c: r.get(3)?,
                t: r.get(4)?,
            },
        ))
    })?;
    for record in records {
        let (day, stats) = record?;
        progress.days.insert(day, stats);
    }
    progress.bookmarks = read_ordered(connection, "bookmarks")?;
    progress.mistakes = read_ordered(connection, "mistakes")?;
    let mut typing = connection
        .prepare("SELECT at,mode,items,wpm,accuracy FROM typing_rounds ORDER BY position")?;
    progress.typing = typing
        .query_map([], |r| {
            Ok(TypingRound {
                at: r.get(0)?,
                mode: r.get(1)?,
                items: r.get(2)?,
                wpm: r.get(3)?,
                accuracy: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    let session: Option<String> = connection
        .query_row("SELECT payload FROM active_session WHERE id=1", [], |r| {
            r.get(0)
        })
        .optional()?;
    progress.session = session
        .map(|raw| serde_json::from_str::<Session>(&raw))
        .transpose()?;
    let mut preferences = serde_json::Map::new();
    let mut settings = connection.prepare("SELECT key,value FROM settings ORDER BY key")?;
    for record in
        settings.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
    {
        let (key, value) = record?;
        preferences.insert(key, serde_json::from_str(&value)?);
    }
    let settings: Settings = serde_json::from_value(serde_json::Value::Object(preferences))?;
    Ok(Some(Snapshot {
        settings,
        progress,
        revision,
    }))
}

fn read_ordered(connection: &Connection, kind: &str) -> Result<Vec<String>, Error> {
    let mut statement =
        connection.prepare("SELECT word_id FROM ordered_words WHERE kind=?1 ORDER BY position")?;
    Ok(statement
        .query_map([kind], |r| r.get(0))?
        .collect::<Result<_, _>>()?)
}

pub(super) fn write_snapshot(
    connection: &Connection,
    previous: Option<&Snapshot>,
    next: &Snapshot,
    record_reviews: bool,
) -> Result<(), Error> {
    connection.execute("INSERT INTO profile(id,dataset_id,progress_version,revision) VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET dataset_id=excluded.dataset_id,progress_version=excluded.progress_version,revision=excluded.revision",params![next.progress.dataset_id,next.progress.version,next.revision])?;
    let empty = BTreeMap::new();
    let old_cards = previous.map_or(&empty, |p| &p.progress.cards);
    write_cards(connection, old_cards, &next.progress, record_reviews)?;
    let empty = BTreeMap::new();
    let old_days = previous.map_or(&empty, |p| &p.progress.days);
    for day in old_days
        .keys()
        .filter(|day| !next.progress.days.contains_key(*day))
    {
        connection.execute("DELETE FROM day_stats WHERE day=?1", [day])?;
    }
    let mut day_write=connection.prepare_cached("INSERT INTO day_stats(day,new_words,reviews,correct,active_seconds) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(day) DO UPDATE SET new_words=excluded.new_words,reviews=excluded.reviews,correct=excluded.correct,active_seconds=excluded.active_seconds")?;
    for (day, stats) in &next.progress.days {
        if old_days.get(day) != Some(stats) {
            day_write.execute(params![day, stats.n, stats.r, stats.c, stats.t])?;
        }
    }
    for (kind, list, old) in [
        (
            "bookmarks",
            &next.progress.bookmarks,
            previous.map(|p| &p.progress.bookmarks),
        ),
        (
            "mistakes",
            &next.progress.mistakes,
            previous.map(|p| &p.progress.mistakes),
        ),
    ] {
        if old != Some(list) {
            write_ordered(connection, kind, list)?;
        }
    }
    write_typing(
        connection,
        previous.map_or(&[], |p| &p.progress.typing),
        &next.progress.typing,
    )?;
    if previous.map(|p| &p.progress.session) != Some(&next.progress.session) {
        match &next.progress.session {
            Some(session) => {
                connection.execute("INSERT INTO active_session(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload",[serde_json::to_string(session)?])?;
            }
            None => {
                connection.execute("DELETE FROM active_session", [])?;
            }
        }
    }
    write_settings(connection, previous.map(|p| &p.settings), &next.settings)?;
    Ok(())
}

fn write_cards(
    connection: &Connection,
    previous: &BTreeMap<String, Card>,
    next: &Progress,
    record_reviews: bool,
) -> Result<(), Error> {
    for id in previous.keys().filter(|id| !next.cards.contains_key(*id)) {
        connection.execute("DELETE FROM cards WHERE word_id=?1", [id])?;
    }
    let mut save=connection.prepare_cached("INSERT INTO cards(word_id,due,stability,difficulty,elapsed_days,scheduled_days,learning_steps,reps,lapses,state,last_review,seen) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(word_id) DO UPDATE SET due=excluded.due,stability=excluded.stability,difficulty=excluded.difficulty,elapsed_days=excluded.elapsed_days,scheduled_days=excluded.scheduled_days,learning_steps=excluded.learning_steps,reps=excluded.reps,lapses=excluded.lapses,state=excluded.state,last_review=excluded.last_review,seen=excluded.seen")?;
    for (id, card) in &next.cards {
        let before = previous.get(id);
        if before == Some(card) {
            continue;
        }
        save.execute(params![
            id,
            card.due,
            card.s,
            card.d,
            card.el,
            card.sd,
            card.ls,
            card.reps,
            card.lapses,
            card.st,
            card.last,
            card.seen
        ])?;
        if record_reviews
            && before.map_or(0, |c| c.reps).checked_add(1) == Some(card.reps)
            && let Some(time) = card.last
            && let Some(grade) = next.session.as_ref().and_then(|s| s.graded.get(id))
        {
            connection.execute("INSERT INTO review_log(word_id,reviewed_at,grade,previous_stability,stability,difficulty,due,reps) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,time,*grade as u8,before.map(|c|c.s),card.s,card.d,card.due,card.reps])?;
        }
    }
    Ok(())
}

fn write_ordered(connection: &Connection, kind: &str, words: &[String]) -> Result<(), Error> {
    connection.execute("DELETE FROM ordered_words WHERE kind=?1", [kind])?;
    let mut insert = connection
        .prepare_cached("INSERT INTO ordered_words(kind,position,word_id) VALUES(?1,?2,?3)")?;
    for (position, id) in words.iter().enumerate() {
        insert.execute(params![kind, position as i64, id])?;
    }
    Ok(())
}

fn write_typing(
    connection: &Connection,
    previous: &[TypingRound],
    next: &[TypingRound],
) -> Result<(), Error> {
    if previous == next {
        return Ok(());
    }
    connection.execute(
        "DELETE FROM typing_rounds WHERE position>=?1",
        [next.len() as i64],
    )?;
    let mut insert=connection.prepare_cached("INSERT INTO typing_rounds(position,at,mode,items,wpm,accuracy) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(position) DO UPDATE SET at=excluded.at,mode=excluded.mode,items=excluded.items,wpm=excluded.wpm,accuracy=excluded.accuracy")?;
    for (position, round) in next.iter().enumerate() {
        if previous.get(position) != Some(round) {
            insert.execute(params![
                position as i64,
                round.at,
                round.mode,
                round.items,
                round.wpm,
                round.accuracy
            ])?;
        }
    }
    Ok(())
}

fn write_settings(
    connection: &Connection,
    previous: Option<&Settings>,
    next: &Settings,
) -> Result<(), Error> {
    if previous == Some(next) {
        return Ok(());
    }
    let old = previous.map(serde_json::to_value).transpose()?;
    let serde_json::Value::Object(values) = serde_json::to_value(next)? else {
        return Err(Error::Invalid("学习设置无法序列化。".into()));
    };
    let mut insert=connection.prepare_cached("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value")?;
    for (key, value) in values {
        if old.as_ref().and_then(|old| old.get(&key)) != Some(&value) {
            insert.execute(params![key, value.to_string()])?;
        }
    }
    Ok(())
}
