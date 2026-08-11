import { writable } from 'svelte/store';
import * as cloudService from '../services/cloudService';
import type { NotebookInvite, NotebookMember, NotebookSummary } from '../services/cloudService';

export const notebooks = writable<NotebookSummary[]>([]);
export const members = writable<NotebookMember[]>([]);
export const invites = writable<NotebookInvite[]>([]);

export async function refreshNotebooks(): Promise<void> {
  notebooks.set(await cloudService.listMyNotebooks());
}

export async function refreshMembers(notebookId: string): Promise<void> {
  members.set(await cloudService.listMembers(notebookId));
}

export async function refreshInvites(notebookId: string): Promise<void> {
  invites.set(await cloudService.listInvites(notebookId));
}

// Called on sign-out and whenever a cloud notebook stops being reachable -
// clears every cloud-scoped list so stale rows from the previous session
// never leak into the UI.
export function resetCloudState(): void {
  notebooks.set([]);
  members.set([]);
  invites.set([]);
}
