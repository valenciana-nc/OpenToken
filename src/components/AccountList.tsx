import { DndContext, KeyboardSensor, PointerSensor, closestCenter, useSensor, useSensors, type Modifier, type KeyboardCoordinateGetter } from '@dnd-kit/core';
import { SortableContext, arrayMove, verticalListSortingStrategy } from '@dnd-kit/sortable';
import type { AccountState } from '../types';
import AccountSection from './AccountSection';
import { providerDefinition } from '../providers';

export function orderAccounts(accounts: AccountState[], order: string[]) {
  const ranks = new Map(order.map((id, i) => [id, i]));
  return [...accounts].sort((a, b) => (ranks.get(a.account.id) ?? Number.MAX_SAFE_INTEGER) - (ranks.get(b.account.id) ?? Number.MAX_SAFE_INTEGER));
}
export function accountKeyboardCoordinates(ids: string[]): KeyboardCoordinateGetter {
  return (event, { context, currentCoordinates }) => {
    if (event.code !== 'ArrowUp' && event.code !== 'ArrowDown') return;
    event.preventDefault();
    if (!context.active || !context.collisionRect) return;
    const current = ids.indexOf(String(context.over?.id ?? context.active.id));
    const target = ids[current + (event.code === 'ArrowDown' ? 1 : -1)];
    const rect = target ? context.droppableRects.get(target) : null;
    if (current < 0 || !rect) return;
    // Choose by list position, not stale animated screen coordinates after cancellation/scroll.
    return { x: currentCoordinates.x, y: rect.top + rect.height / 2 - context.collisionRect.height / 2 };
  };
}
const vertical: Modifier = ({ transform }) => ({ ...transform, x: 0 });
export default function AccountList({ accounts, order, now, example, saving, reorder }: { accounts: AccountState[]; order: string[]; now: number; example: boolean; saving: boolean; reorder: (order: string[]) => Promise<void> }) {
  const sorted = orderAccounts(accounts, order);
  const ids = sorted.map(s => s.account.id);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }), useSensor(KeyboardSensor, { coordinateGetter: accountKeyboardCoordinates(ids), scrollBehavior: 'auto' }));
  const label = (id: string | number) => sorted.find(s => s.account.id === id)?.account.label ?? 'account';
  return <DndContext sensors={sensors} collisionDetection={closestCenter} modifiers={[vertical]}
    accessibility={{ screenReaderInstructions: { draggable: 'To reorder this account, press Space or Enter, use Up and Down, then press Space or Enter to save. Escape cancels.' }, announcements: {
      onDragStart: ({ active }) => `Picked up ${label(active.id)}. Use Up and Down to move.`,
      onDragOver: ({ active, over }) => over ? `${label(active.id)}, position ${ids.indexOf(String(over.id)) + 1} of ${ids.length}.` : undefined,
      onDragEnd: ({ active, over }) => over ? `Moved ${label(active.id)} to position ${ids.indexOf(String(over.id)) + 1}.` : 'Move cancelled.',
      onDragCancel: () => 'Move cancelled. Account order unchanged.',
    } }}
    onDragEnd={({ active, over }) => {
      if (over && active.id !== over.id && !saving) {
        const from = ids.indexOf(String(active.id)), to = ids.indexOf(String(over.id));
        if (from >= 0 && to >= 0) void reorder(arrayMove(ids, from, to));
      }
    }}>
    <SortableContext items={ids} strategy={verticalListSortingStrategy}>
      <div className="account-sections">{sorted.map(state => <AccountSection key={state.account.id} state={state} now={now} example={example} heading={ids.length > 1 || !!state.account.email || state.account.label !== providerDefinition(state.account.provider).name} reorderDisabled={ids.length < 2} reorderSaving={saving}/>)}</div>
    </SortableContext>
  </DndContext>;
}
