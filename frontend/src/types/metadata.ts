export interface StaffMember {
  role: string;
  name: string;
}

export interface MetadataDraft {
  title: string;
  originalTitle?: string;
  developer?: string;
  releaseDate?: string;
  description?: string;
  tags?: string[];
  rating?: number;
  playStatus?: string;
  staff?: StaffMember[];
}

export interface VndbMetadata {
  title?: string;
  original_title?: string;
  developer?: string;
  release_date?: string;
  description?: string;
  tags?: string[];
  staff?: StaffMember[];
  [key: string]: unknown;
}
