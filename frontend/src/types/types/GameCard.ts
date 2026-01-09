// Type definition matching the data schema
export type EntryType = 'canonical' | 'suggested' | 'orphan';

export interface GameCardData {
  entry_type: EntryType;
  display_title: string;
  cover_image?: string;
  instance_count?: number;
  actions_allowed: 'NONE';
}
