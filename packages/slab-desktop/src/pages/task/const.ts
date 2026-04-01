export const PAGE_SIZE = 4;

export interface Task {
  id: string;
  status: string;
  task_type: string;
  error_msg?: string | null;
  created_at: string;
  updated_at: string;
}

export interface TaskResult {
  [key: string]: any;
}
