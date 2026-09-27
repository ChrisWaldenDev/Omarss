export type DialogState =
  | { kind: "addFeed" }
  | { kind: "editFeed"; feedId: number }
  | { kind: "folder"; folderId: number | null; name: string }
  | {
      kind: "confirm";
      title: string;
      message: string;
      confirmLabel: string;
      onConfirm: () => Promise<void>;
    };

/** The one modal dialog that can be open at a time. */
export class DialogsStore {
  current = $state<DialogState | null>(null);

  open(dialog: DialogState): void {
    this.current = dialog;
  }

  close(): void {
    this.current = null;
  }
}

export const dialogs = new DialogsStore();
