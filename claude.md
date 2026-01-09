# Claude Code Working Instructions

## 🎨 Frontend Design Rules (CRITICAL)

When working on **ANY frontend design or UI/UX task**, you **MUST** automatically activate and use these skills:

### Required Skills (Auto-Activate)

1. **UI/UX Pro Max** (`ui-ux-pro-max`)
2. **Frontend Design** (`frontend-design`)

### When to Activate

**ALWAYS** use these skills when:
- Designing new UI components
- Creating new pages/views
- Implementing visual features
- Working on layout, styling, or user experience
- Analyzing or improving existing UI
- Converting designs to code

### Activation Command

```bash
# When starting ANY frontend design task:
Skill: ui-ux-pro-max
Skill: frontend-design
```

### Why This Matters

These skills provide:
- ✅ Professional design patterns
- ✅ Color palette recommendations
- ✅ Typography pairings with Google Fonts imports
- ✅ Stack-specific best practices (React, Tailwind, etc.)
- ✅ UX guidelines and anti-patterns
- ✅ Accessibility standards
- ✅ Production-quality code generation

### Never Skip These Skills

**DO NOT**:
- ❌ Write UI code without activating these skills first
- ❌ Guess design tokens, colors, or fonts
- ❌ Use emoji as icons (use SVG icons from Heroicons/Lucide)
- ❌ Ignore hover states and accessibility
- ❌ Create layouts without proper spacing hierarchy

**ALWAYS**:
- ✅ Search the skill database before implementing
- ✅ Follow the skill's output exactly
- ✅ Use recommended color palettes and font pairings
- ✅ Implement proper hover/active/focus states
- ✅ Ensure cursor-pointer on all interactive elements

---

## 🚀 Quick Reference

### Typical Workflow

```
User: "Create a new dashboard page"

Claude:
1. Activate ui-ux-pro-max skill
2. Activate frontend-design skill
3. Search relevant domains (product, style, color, typography)
4. Synthesize design system
5. Implement code following skill recommendations
```

### Skill Search Examples

```bash
# Before implementing:
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "dashboard" --domain product
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "minimal" --domain style
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "modern" --domain typography
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "fintech" --domain color
python3 .claude/skills/ui-ux-pro-max/scripts/search.py "responsive" --stack html-tailwind
```

---

## 📋 Project-Specific Notes

### Galroon - Visual Novel Library

**Stack**: React + TypeScript + Tailwind CSS + Vite

**Design System**:
- Dark theme: `#0B0C0F` background
- Primary text: `#FFFFFF` / `#B3B3B3` / `#6B6B6B`
- Accent: `#7BA8C7` (azure), `#FF9100` (gold)
- Border radius: 4px (buttons), 12px (cards)
- Font: Inter (primary), Noto Sans TC (Chinese support)

**Key Pages**:
- Gallery (grid view with hero carousel)
- Work Details (encyclopedia-style game info)
- Characters Page (VNDB-inspired character list)
- Dashboard (analytics and stats)

**Component Patterns**:
- Sidebar navigation (240px fixed)
- Top bar with search
- Card-based layouts
- Expandable content sections
- CV blocks with avatars (44px circular)

---

## 🎯 Quality Checklist

Before delivering any UI code, verify:

### Visual Quality
- [ ] UI/UX Pro Max skill was used
- [ ] Frontend Design skill was used
- [ ] No emoji icons (use SVG)
- [ ] Consistent icon sizing (24x24 viewBox)
- [ ] Stable hover states (no layout shift)
- [ ] Proper color contrast (4.5:1 minimum)

### Interaction
- [ ] `cursor-pointer` on all clickable elements
- [ ] Hover states provide feedback
- [ ] Smooth transitions (150-300ms)
- [ ] Focus states for keyboard navigation

### Accessibility
- [ ] All images have alt text
- [ ] Form inputs have labels
- [ ] Color not the only indicator
- [ ] `prefers-reduced-motion` respected

---

## 📝 Last Updated

**Date**: 2025-01-07
**Updated by**: Claude (Sonnet 4.5)
**Reason**: Established mandatory frontend design workflow using UI/UX Pro Max & Frontend Design skills
