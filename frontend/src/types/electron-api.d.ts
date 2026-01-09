export interface ElectronSessionTokenResponse {
  success: boolean;
  token?: string;
}

export interface ElectronApiPortResponse {
  success: boolean;
  port: number;
}

export interface ElectronAuthApi {
  getSessionToken: () => Promise<ElectronSessionTokenResponse>;
  getApiPort: () => Promise<ElectronApiPortResponse>;
}

export interface ElectronAPI {
  auth: ElectronAuthApi;
}

declare global {
  interface Window {
    electronAPI?: ElectronAPI;
  }
}

export {};
