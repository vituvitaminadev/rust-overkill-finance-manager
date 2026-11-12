import { useState } from "react";
import { FormError } from "@/components/Field";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  api,
  errorMessage,
  type InstallmentPlan,
  type Recurrence,
} from "@/lib/api";
import { formatMoney, monthLabel } from "@/lib/format";
import {
  useHouseholdMutation,
  useInstallmentPlans,
  useRecurrences,
} from "@/lib/queries";

/** As Recorrências em vigor e os Parcelamentos com parcelas a vencer. */
export function PlanningView() {
  const recurrences = useRecurrences();
  const plans = useInstallmentPlans();
  const [ending, setEnding] = useState<Recurrence | null>(null);
  const [deleting, setDeleting] = useState<InstallmentPlan | null>(null);
  const error = recurrences.error ?? plans.error;

  return (
    <div className="grid gap-4">
      <p className="text-sm text-muted-foreground">
        Para criar uma recorrência ou um parcelamento, use “Novo lançamento” e
        escolha a repetição.
      </p>
      <FormError error={error && errorMessage(error)} />

      <Card>
        <CardHeader>
          <CardTitle>Recorrências</CardTitle>
          <CardDescription>
            Geram um lançamento previsto por mês, sempre doze meses à frente.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {recurrences.data?.length === 0 ? (
            <p className="py-2 text-sm text-muted-foreground">
              Nenhuma recorrência em vigor.
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Descrição</TableHead>
                  <TableHead>Categoria</TableHead>
                  <TableHead>Responsável</TableHead>
                  <TableHead>Quando</TableHead>
                  <TableHead className="text-right">Valor</TableHead>
                  <TableHead className="w-24" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {(recurrences.data ?? []).map((recurrence) => (
                  <TableRow key={recurrence.id}>
                    <TableCell>{recurrence.description}</TableCell>
                    <TableCell>{recurrence.categoryName}</TableCell>
                    <TableCell>{recurrence.assigneeName}</TableCell>
                    <TableCell>
                      Todo dia {recurrence.dayOfMonth}
                      {recurrence.endMonth &&
                        `, até ${monthLabel(recurrence.endMonth)}`}
                    </TableCell>
                    <TableCell className="text-right font-medium tabular-nums">
                      {recurrence.kind === "expense" ? "−" : "+"}
                      {formatMoney(recurrence.amountCents)}
                    </TableCell>
                    <TableCell className="text-right">
                      {recurrence.canEdit && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setEnding(recurrence)}
                        >
                          Encerrar
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Parcelamentos</CardTitle>
          <CardDescription>
            Compras divididas em parcelas mensais que ainda têm algo a vencer.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {plans.data?.length === 0 ? (
            <p className="py-2 text-sm text-muted-foreground">
              Nenhum parcelamento em aberto.
            </p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Descrição</TableHead>
                  <TableHead>Categoria</TableHead>
                  <TableHead>Responsável</TableHead>
                  <TableHead>Parcelas a vencer</TableHead>
                  <TableHead className="text-right">Falta</TableHead>
                  <TableHead className="text-right">Total</TableHead>
                  <TableHead className="w-24" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {(plans.data ?? []).map((plan) => (
                  <TableRow key={plan.id}>
                    <TableCell>{plan.description}</TableCell>
                    <TableCell>{plan.categoryName}</TableCell>
                    <TableCell>{plan.assigneeName}</TableCell>
                    <TableCell className="tabular-nums">
                      {plan.plannedCount} de {plan.count}
                    </TableCell>
                    <TableCell className="text-right font-medium tabular-nums">
                      {formatMoney(plan.plannedCents)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">
                      {formatMoney(plan.totalCents)}
                    </TableCell>
                    <TableCell className="text-right">
                      {plan.canEdit && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setDeleting(plan)}
                        >
                          Excluir
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {ending && (
        <ConfirmDialog
          title="Encerrar recorrência?"
          description={`Os lançamentos previstos de “${ending.description}” com data futura serão removidos. Os efetivados e os atrasados ficam como estão.`}
          confirmLabel="Encerrar"
          action={() => api.endRecurrence(ending.id)}
          onClose={() => setEnding(null)}
        />
      )}
      {deleting && (
        <ConfirmDialog
          title="Excluir parcelamento?"
          description={`As ${deleting.plannedCount} parcelas ainda previstas de “${deleting.description}” serão removidas. As já efetivadas ficam como estão.`}
          confirmLabel="Excluir"
          action={() => api.deleteInstallmentPlan(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </div>
  );
}

function ConfirmDialog({
  title,
  description,
  confirmLabel,
  action,
  onClose,
}: {
  title: string;
  description: string;
  confirmLabel: string;
  action: () => Promise<void>;
  onClose: () => void;
}) {
  const confirm = useHouseholdMutation(action, onClose);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <FormError error={confirm.error && errorMessage(confirm.error)} />
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancelar
          </Button>
          <Button
            variant="destructive"
            disabled={confirm.isPending}
            onClick={() => confirm.mutate(undefined)}
          >
            {confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
