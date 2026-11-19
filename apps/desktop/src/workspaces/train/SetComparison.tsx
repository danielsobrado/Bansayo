import type { ClinicAttempt, ClinicReview } from "@contracts";
import { useQuery } from "../../engine/queries";
import { paths } from "../../engine/trainingActions";
import { MeasurementValue } from "../../ui/MeasurementValue";
import { nameOf, useContentIndex } from "./common";
import styles from "./train.module.css";

export function SetComparison({ sessionId }: { sessionId: string }) {
  const index = useContentIndex();
  const review = useQuery<ClinicReview>(paths.clinicReview(sessionId)).data;
  if (!review?.second) return null;
  const { first, second } = review;

  const link = (attempt: ClinicAttempt) => `#/technique/${sessionId}/${attempt.rep_index}`;
  return (
    <section className={styles.card} aria-label="Set comparison">
      <h2>Set comparison · {review.side}</h2>
      <p className={styles.muted}>First completed attempt from each set.</p>
      <div className={styles.comparisonScroll}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th scope="col">Measurement</th>
              <th scope="col"><a href={link(first)}>Set {first.set_index} · Attempt {first.rep_index}</a></th>
              <th scope="col"><a href={link(second)}>Set {second.set_index} · Attempt {second.rep_index}</a></th>
            </tr>
          </thead>
          <tbody>
            {review.measurements.map(({ metric, first: a, second: b, comparable }) => (
                <tr key={metric}>
                  <th scope="row">
                    {nameOf(index, metric)}
                    {!comparable && <div className={styles.muted}>Comparison unavailable</div>}
                  </th>
                  <td>{a ? <MeasurementValue m={a} /> : "—"}</td>
                  <td>{b ? <MeasurementValue m={b} /> : "—"}</td>
                </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
