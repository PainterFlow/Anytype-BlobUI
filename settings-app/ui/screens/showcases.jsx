import React, { useId, useState } from "react";
import { HelpTooltip } from "../shared/help-tooltip.jsx";
import "../showcases.css";

const weekDays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const sampleYear = 2026;
const sampleMonth = 9;
const monthStart = new Date(Date.UTC(sampleYear, sampleMonth, 1));
const daysBeforeMonth = (monthStart.getUTCDay() + 6) % 7;
const monthLabel = new Intl.DateTimeFormat("en-US", {
  month: "long", year: "numeric", timeZone: "UTC",
}).format(monthStart);
const dateFormatter = new Intl.DateTimeFormat("en-US", { dateStyle: "long", timeZone: "UTC" });
const weeks = Array.from({ length: 6 }, (_, week) => Array.from({ length: 7 }, (_, day) => {
  return new Date(Date.UTC(sampleYear, sampleMonth, week * 7 + day + 1 - daysBeforeMonth));
}));

const eventsByDate = {
  1: [{ title: "Fold kite", color: "blue" }],
  3: [{ title: "Test ink", color: "purple" }],
  5: [{ title: "Bake tart", color: "amber" }, { title: "Press ferns", color: "green" }],
  9: [{ title: "Trace a star", color: "purple" }],
  12: [
    { title: "Cut frame", color: "blue" },
    { title: "Mix paint", color: "amber" },
    { title: "Stamp stars", color: "purple" },
    { title: "Dry herbs", color: "green" },
  ],
  15: [{ title: "Sketch vase", color: "blue" }],
  17: [{ title: "Roll wax", color: "amber" }],
  20: [{ title: "Seed packet", color: "green" }],
  23: [{ title: "Paint beads", color: "purple" }, { title: "Curl ribbon", color: "blue" }],
  26: [{ title: "Ink a block", color: "purple" }],
  30: [{ title: "Polish brass", color: "amber" }],
};

function CheckboxGlyph({ checked = false }) {
  return <svg className="showcase-checkbox" viewBox="0 0 14 14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.2">
    <rect x="2" y="2" width="10" height="10" rx="2" />
    {checked && <path d="m4.1 7 1.9 1.9 3.9-3.9" strokeLinecap="round" strokeLinejoin="round" />}
  </svg>;
}

function CalendarDay({ date, expanded, onToggle, idPrefix }) {
  const day = date.getUTCDate();
  const spillover = date.getUTCMonth() !== sampleMonth;
  const dateLabel = dateFormatter.format(date);
  const events = spillover ? [] : eventsByDate[day] ?? [];
  const eventListId = `${idPrefix}-day-${day}-events`;
  const hiddenCount = Math.max(0, events.length - 2);
  const visibleEvents = expanded ? events : events.slice(0, 2);

  return <div className={`calendar-showcase-day${spillover ? " calendar-showcase-spillover" : ""}`} role="gridcell" aria-label={dateLabel}>
    <span className="calendar-showcase-date">{day}</span>
    {events.length > 0 && <div id={eventListId} className="calendar-showcase-events">
      {visibleEvents.map(event => <div className={`calendar-showcase-event showcase-${event.color}`} key={event.title} title={event.title}>
        <CheckboxGlyph />
        <span>{event.title}</span>
      </div>)}
    </div>}
    {hiddenCount > 0 && <button type="button" className="calendar-showcase-more" aria-expanded={expanded}
      aria-label={expanded ? `Show fewer events for ${dateLabel}` : `Show ${hiddenCount} more events for ${dateLabel}`}
      aria-controls={eventListId} onClick={() => onToggle(day)}>
      {expanded ? "Show less" : `+${hiddenCount} more`}
    </button>}
  </div>;
}

export function CalendarShowcase({ compact = false }) {
  const idPrefix = `calendar-showcase-${useId()}`;
  const [expandedDates, setExpandedDates] = useState(() => new Set());
  const toggleDate = day => setExpandedDates(previous => {
    const next = new Set(previous);
    if (next.has(day)) next.delete(day);
    else next.add(day);
    return next;
  });

  return <section className={`calendar-showcase${compact ? " calendar-showcase-compact" : ""}`} aria-label="Calendar showcase">
    <header className="showcase-heading-row">
      <h3>{monthLabel}</h3>
      <HelpTooltip label="Calendar showcase help"
        text="Sample events show fixed property colors. Expand a day to see the rest of its events." />
    </header>
    <div className="calendar-showcase-grid" role="grid" aria-label={`${monthLabel} sample calendar`}>
      <div className="calendar-showcase-week" role="row">
        {weekDays.map(day => <span className="calendar-showcase-weekday" role="columnheader" key={day}>{day}</span>)}
      </div>
      {weeks.map(week => <div className="calendar-showcase-week" role="row" key={week[0].getTime()}>
        {week.map(date => <CalendarDay key={date.getTime()} date={date} idPrefix={idPrefix}
          expanded={date.getUTCMonth() === sampleMonth && expandedDates.has(date.getUTCDate())} onToggle={toggleDate} />)}
      </div>)}
    </div>
  </section>;
}

const kanbanColumns = [
  { name: "Collect", color: "blue", cards: [
    { title: "Gather pebbles", minutes: 15, day: 3 },
    { title: "Pick paper scraps", minutes: 10 },
    { title: "Find cork sheet", minutes: 10 },
  ] },
  { name: "Arrange", color: "purple", cards: [
    { title: "Lay out paper shapes", minutes: 20, day: 7 },
    { title: "Sort ribbons", minutes: 10 },
  ] },
  { name: "Make", color: "amber", cards: [
    { title: "Knot tassel", minutes: 20, day: 12 },
    { title: "Felt coaster", minutes: 25 },
    { title: "Leaf print", minutes: 15 },
  ] },
  { name: "Review", color: "pink", cards: [
    { title: "Check folds", minutes: 10, day: 16 },
    { title: "Compare wax", minutes: 15 },
  ] },
  { name: "Store", color: "green", complete: true, cards: [
    { title: "Label bead jar", minutes: 10, day: 20 },
    { title: "Stack stencils", minutes: 5 },
  ] },
];

export function KanbanShowcase({ layout = "default", formulaDisplay = "balanced", collapsedCategories = false, compact = false }) {
  return <section className={`kanban-showcase${compact ? " kanban-showcase-compact" : ""}`} aria-label="Kanban showcase">
    <header className="showcase-heading-row">
      <h3>Craft board</h3>
      <HelpTooltip label="Kanban showcase help"
        text="Each category shows the total Focus minutes for its sample cards. The preview follows your board layout, formula placement, and collapsed categories." />
    </header>
    <div className="kanban-showcase-board" data-layout={layout} data-formula-display={formulaDisplay}
      data-collapsed={String(Boolean(collapsedCategories))}>
      {kanbanColumns.map(column => <section className={`kanban-showcase-column showcase-${column.color}`} key={column.name}
        tabIndex={0} aria-label={`${column.name} category`}>
        <header className="kanban-showcase-column-heading">
          <span className="kanban-showcase-category">
            <span className="kanban-showcase-column-name">{column.name}</span>
            <span className="kanban-showcase-count">{column.cards.length}</span>
          </span>
          <span className="kanban-showcase-total" title="Total Focus minutes">
            <span className="sr-only">Total Focus minutes </span>
            {column.cards.reduce((total, card) => total + card.minutes, 0)} min
          </span>
        </header>
        <div className="kanban-showcase-cards">
          {column.cards.map(card => <article className="kanban-showcase-card" key={card.title}>
            <div className="kanban-showcase-card-heading">
              <CheckboxGlyph checked={column.complete} />
              <span className="sr-only">{column.complete ? "Complete" : "Incomplete"}</span>
              <span className="kanban-showcase-card-title" title={card.title}>{card.title}</span>
            </div>
            <div className="kanban-showcase-card-meta">
              <span className="kanban-showcase-chip" title="Focus minutes">{card.minutes} min</span>
              {card.day && <time className="kanban-showcase-chip" dateTime={`2026-10-${String(card.day).padStart(2, "0")}`}>Oct {card.day}</time>}
            </div>
          </article>)}
          <div className="kanban-showcase-add" aria-hidden="true">
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round">
              <path d="M7 3v8M3 7h8" />
            </svg>
            <span>New card</span>
          </div>
        </div>
      </section>)}
    </div>
  </section>;
}
