// Project data model of the local app.
// These shapes are inherited from the original web version (PocketBase
// collections) so the editor and dashboard code keeps working unchanged.

export interface FileRecord {
  id: string;
  file_name: string;
  original_name: string;
  project_name?: string;
  file_size: number;
  file_type: string;
  ecu_type?: string;
  hardware_version?: string;
  software_version?: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  maps_detected: number;
  detection_data?: any;
  vehicle_brand?: string;
  vehicle_model?: string;
  engine_type?: string;
  transmission_type?: string;
  year?: string;
  power?: string;
  customer?: string;
  stage?: string;
  date?: string;
  notes?: string;
  map_display_settings?: any;
  created: string;
  updated: string;
}

export interface Version {
  id: string;
  file: string; // file ID
  name: string;
  is_current: boolean;
  base_version?: string | null; // base version ID
  created: string;
}

export interface MapEdit {
  id: string;
  version: string; // version ID
  map_address: number;
  payload: any;
  created: string;
}
