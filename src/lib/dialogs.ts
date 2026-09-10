import { ask } from '@tauri-apps/plugin-dialog'

/** Native NSAlert sheets, worded the way macOS words them: verb on the button, not "OK". */
export const confirmDeleteMeeting = (title: string) =>
  ask(`This removes the transcript and every audio clip from “${title}”. You can’t undo this.`, {
    title: 'Delete this meeting?',
    kind: 'warning',
    okLabel: 'Delete',
    cancelLabel: 'Cancel',
  })

export const confirmForgetVoices = () =>
  ask('People will be labelled “Speaker 1, 2…” again until you assign them.', {
    title: 'Forget every stored voice?',
    kind: 'warning',
    okLabel: 'Forget Voices',
    cancelLabel: 'Cancel',
  })
