import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { PlanSidebar } from './PlanSidebar'

describe('PlanSidebar (issue #1434 plan dock)', () => {
  const entries = [
    { content: 'Investigate renderer bypass', status: 'completed' },
    { content: 'Fix the delegation', status: 'in_progress' }
  ]

  it('renders the live plan as a docked column with the panel rows', () => {
    const { container } = render(<PlanSidebar entries={entries} />)

    const sidebar = container.querySelector('[data-plan-sidebar="true"]')
    expect(sidebar).toBeInTheDocument()
    // Docked column: fixed width beside the chat stream, hidden on panes the
    // chat `@container` reports narrower than 720px (the inline fallback bar
    // covers those).
    expect(sidebar).toHaveClass('w-72')
    expect(sidebar).toHaveClass('@max-[719px]:hidden')

    // The rows come from the docked PlanPanel: header toggle + entries.
    expect(
      screen.getByRole('button', { name: /Plan, 1 of 2 tasks, task in progress/ })
    ).toBeInTheDocument()
    expect(screen.getByText('Investigate renderer bypass')).toBeInTheDocument()
    expect(screen.getByText('Fix the delegation')).toBeInTheDocument()
  })

  it('renders nothing without entries (chat feed stays clean, no empty column)', () => {
    const { container } = render(<PlanSidebar entries={[]} />)
    expect(container.querySelector('[data-plan-sidebar="true"]')).toBeNull()
  })
})
