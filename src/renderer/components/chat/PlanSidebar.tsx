import type { PlanEntry } from '@/lib/acp-api'
import { cn } from '@/lib/utils'
import { PlanPanel } from './PlanPanel'

interface PlanSidebarProps {
  /** Live session plan (`acp:plan_update`). Empty unmounts the column. */
  entries: PlanEntry[]
  className?: string
}

/**
 * Plan dock (issue #1434): the live execution plan in a dedicated right-side
 * column beside the chat stream, instead of an inline block that consumes
 * vertical space in the message feed. Wide panes (chat `@container` ≥ 720px)
 * get the column; narrower panes and the phone shell keep the inline
 * `PlanPanel` bar, so no surface loses the plan.
 *
 * The column is a thin owner for the pane-width gate and the divider; the
 * rows, status icons, priority badges and the collapse toggle come from
 * {@link PlanPanel} in its `dock` variant, so the live plan and the
 * historical `termul-plan` fence snapshots render identically.
 */
export function PlanSidebar({ entries, className }: PlanSidebarProps): React.JSX.Element | null {
  if (entries.length === 0) return null
  return (
    <aside
      data-plan-sidebar="true"
      className={cn(
        '@max-[719px]:hidden flex w-72 min-w-0 shrink-0 flex-col overflow-hidden',
        className
      )}
    >
      <PlanPanel entries={entries} variant="dock" />
    </aside>
  )
}
