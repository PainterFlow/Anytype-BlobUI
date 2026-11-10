import React, { useId, useState } from "react";
import { HelpTooltip } from "../shared/help-tooltip.jsx";
import "../showcases.css";

const weekDays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const eventsByDate = {
  1: [{ title: "Project kickoff", color: "work" }, { title: "Sprint plan", color: "work" }],
  3: [{ title: "Dentist", color: "health" }],
  4: [{ title: "Read a chapter", color: "personal" }],
  8: [{ title: "Design review", color: "work" }],
  10: [{ title: "Plant the herbs", color: "home" }],
  12: [
    { title: "Showcase review", color: "work" },
    { title: "Lunch with Mira", color: "personal" },
    { title: "Pick up a parcel", color: "home" },
    { title: "Evening walk", color: "health" },
  ],
  16: [{ title: "Portfolio draft", color: "personal" }],
  18: [{ title: "Team retro", color: "work" }, { title: "Book club", color: "personal" }],
  22: [{ title: "Weekly planning", color: "work" }],
  24: [{ title: "Gallery visit", color: "personal" }, { title: "Gym", color: "health" }],
  29: [{ title: "Release prep", color: "work" }],
  30: [{ title: "Tidy the desk", color: "home" }],
};
const eventLegend = [
  ["work", "Work"], ["personal", "Personal"], ["home", "Home"], ["health", "Health"],
];

function CalendarDay({ date, expanded, onToggle, idPrefix }) {
  const events = eventsByDate[date] || [];
  const eventListId = `${idPrefix}-day-${date}-events`;
  const visibleEvents = expanded ? events : events.slice(0, 2);
  const hiddenCount = Math.max(0, events.length - visibleEvents.length);
  return <div className={`calendar-showcase-day${date === 12 ? " has-overflow" : ""}`} role="gridcell" aria-label={`June ${date}, 2026`}>
    <span className="calendar-showcase-date">{date}</span>
    <div id={eventListId} className="calendar-showcase-events">
      {visibleEvents.map((event, index) => <div className={`calendar-showcase-event event-${event.color}`} key={`${event.title}-${index}`} title={event.title}>
        <span className="calendar-showcase-event-dot" aria-hidden="true" />
        <span>{event.title}</span>
      </div>)}
    </div>
    {hiddenCount > 0 && <button type="button" className="calendar-showcase-more" aria-expanded="false"
      aria-label={`Show ${hiddenCount} more events for June ${date}`} aria-controls={eventListId} onClick={() => onToggle(date)}>
      +{hiddenCount} more
    </button>}
    {expanded && events.length > 2 && <button type="button" className="calendar-showcase-more" aria-expanded="true"
      aria-label={`Show fewer events for June ${date}`} aria-controls={eventListId} onClick={() => onToggle(date)}>
      Show less
    </button>}
  </div>;
}

export function CalendarShowcase({ compact = false }) {
  const idPrefix = `calendar-showcase-${useId().replace(/:/g, "")}`;
  const [expandedDates, setExpandedDates] = useState(() => new Set());
  const weeks = Array.from({ length: 5 }, (_, week) => Array.from({ length: 7 }, (_, day) => {
    const date = week * 7 + day + 1;
    return date <= 30 ? date : null;
  }));
  const toggleDate = date => setExpandedDates(previous => {
    const next = new Set(previous);
    if (next.has(date)) next.delete(date);
    else next.add(date);
    return next;
  });

  return <section className={`calendar-showcase${compact ? " calendar-showcase-compact" : ""}`} aria-label="Calendar showcase">
    <header className="showcase-heading-row">
      <h3>June 2026</h3>
      <HelpTooltip label="Calendar showcase help"
        text="Sample events show fixed property colors. Expand a day to see events that do not fit." />
    </header>
    <ul className="calendar-showcase-legend" aria-label="Sample color values">
      {eventLegend.map(([color, label]) => <li key={color}><span className={`calendar-showcase-legend-dot event-${color}`} aria-hidden="true" />{label}</li>)}
    </ul>
    <div className="calendar-showcase-grid" role="grid" aria-label="June 2026 sample calendar">
      <div className="calendar-showcase-week calendar-showcase-header" role="row">
        {weekDays.map(day => <span className="calendar-showcase-weekday" role="columnheader" key={day}>{day}</span>)}
      </div>
      {weeks.map((week, weekIndex) => <div className="calendar-showcase-week" role="row" key={`week-${weekIndex}`}>
        {week.map((date, dayIndex) => date
          ? <CalendarDay key={date} date={date} expanded={expandedDates.has(date)} onToggle={toggleDate} idPrefix={idPrefix} />
          : <span key={`empty-${dayIndex}`} className="calendar-showcase-day calendar-showcase-empty" role="gridcell" aria-label="Outside June" />)}
      </div>)}
    </div>
  </section>;
}

const kanbanColumns = [
  {
    name: "To do", total: 10, cards: [
      { title: "Finish the guide", kind: "Docs", points: 3 },
      { title: "Review install steps", kind: "Build", points: 5 },
      { title: "Check keyboard help", kind: "UI", points: 2 },
    ],
  },
  {
    name: "In progress", total: 16, cards: [
      { title: "Calendar example", kind: "UI", points: 8 },
      { title: "Board previews", kind: "UI", points: 5 },
      { title: "Package guide", kind: "Build", points: 3 },
    ],
  },
  {
    name: "Done", total: 15, cards: [
      { title: "Help button", kind: "UI", points: 2 },
      { title: "Replace Docs menu", kind: "Docs", points: 5 },
      { title: "Update catalog", kind: "UI", points: 8 },
    ],
  },
];

const sampleFormula = String.raw`\property{"Points"}{"sum"}`;

export function KanbanShowcase({ layout = "default", formulaDisplay = "balanced", collapsedCategories = false, compact = false }) {
  return <section className={`kanban-showcase${compact ? " kanban-showcase-compact" : ""}`} aria-label="Kanban showcase">
    <header className="showcase-heading-row kanban-showcase-heading">
      <div>
        <h3>Project board</h3>
        <code className="kanban-showcase-formula" title={sampleFormula}>{sampleFormula}</code>
      </div>
      <HelpTooltip label="Kanban showcase help"
        text="Sample cards use a Points sum. The preview follows your board layout, formula placement, and collapsed categories." />
    </header>
    <div className="kanban-showcase-board" data-layout={layout} data-formula-display={formulaDisplay}
      data-collapsed={String(Boolean(collapsedCategories))}>
      {kanbanColumns.map(column => <section className="kanban-showcase-column" key={column.name} tabIndex={0} aria-label={`${column.name} category`}>
        <header className="kanban-showcase-column-heading">
          <span className="kanban-showcase-column-name">{column.name}<span className="kanban-showcase-count">{column.cards.length}</span></span>
          <span className="kanban-showcase-total">{column.total} pts</span>
        </header>
        <div className="kanban-showcase-cards">
          {column.cards.map(card => <article className="kanban-showcase-card" key={card.title}>
            <span className="kanban-showcase-card-title">{card.title}</span>
            <span className="kanban-showcase-card-meta"><span className={`kanban-showcase-tag tag-${card.kind.toLowerCase()}`}>{card.kind}</span><span>{card.points} pts</span></span>
          </article>)}
        </div>
      </section>)}
    </div>
  </section>;
}
