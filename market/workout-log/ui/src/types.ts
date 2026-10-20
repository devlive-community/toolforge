export type Mode = 'external' | 'bodyweight' | 'unknown'
export type Unit = 'kg' | 'lb'
export type Status = 'pending' | 'done' | 'skipped'
export interface Exercise { id: string; name: string; equipment: string; mode: Mode; archived: boolean; history: number }
export interface SetRecord { reps: string; weight: string; unit: Unit; warmup: boolean; note: string; status: Status }
export interface Entry { exercise: Exercise; sets: SetRecord[] }
export interface Rest { started: number; seconds: number; stopped: boolean }
export interface Body { name: string; date: string; note: string; finished: boolean; entries: Entry[]; rest: Rest | null }
export interface Workout { id: number; body: Body; undo: Body | null }
export interface Summary { completed: number; pending: number; reps: number; entries: { exercise: Exercise; sets: { index: number; reps: string; weight: string; kg: string | null; unit: Unit; warmup: boolean; note: string }[] }[] }
export interface WorkoutView { workout: Workout; summary: Summary }
export interface Template { id: string; name: string; entries: Entry[] }
export interface Home { exercises: Exercise[]; exerciseTotal: number; templates: Template[]; templateTotal: number; choices: Exercise[]; historyChoices: Pick<Exercise, 'id' | 'name' | 'equipment' | 'archived'>[]; templateChoices: { id: string; name: string }[]; workoutTotal: number; today: string }
export interface Listing { rows: WorkoutView[]; total: number; offset: number; limit: number }
