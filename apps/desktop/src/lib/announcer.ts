/** Messages for the polite live region. Each announcement gets a new id so repeats are re-read. */
import { createStore } from "../engine/store";

export interface Announcement {
  id: number;
  text: string;
}

let nextId = 1;
export const announcementStore = createStore<Announcement | null>(null);

export function announce(text: string): void {
  announcementStore.set({ id: nextId++, text });
}
