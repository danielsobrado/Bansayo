import { Dialog } from "./Dialog";
import type { DialogProps } from "./Dialog";
import styles from "./Drawer.module.css";
export type DrawerProps = DialogProps;
/** A modal inspector with the same keyboard ownership as Dialog. */
export function Drawer({ className, ...props }: DrawerProps) {
  return <Dialog {...props} className={[styles.drawer, className].filter(Boolean).join(" ")} />;
}
