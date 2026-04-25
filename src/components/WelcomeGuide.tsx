import React from "react";
import { useTranslation } from "react-i18next";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Route, CheckSquare, Zap, ShieldCheck } from "lucide-react";

interface WelcomeGuideProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDismiss: (dontShowAgain: boolean) => void;
}

const STEP_ICONS = [ShieldCheck, Route, CheckSquare, Zap];

export function WelcomeGuide({ open, onOpenChange, onDismiss }: WelcomeGuideProps) {
  const { t } = useTranslation();
  const [dontShowAgain, setDontShowAgain] = React.useState(false);

  const handleClose = () => {
    onDismiss(dontShowAgain);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="text-lg">{t("guide.title")}</DialogTitle>
          <p className="text-sm text-muted-foreground mt-1">
            {t("guide.description")}
          </p>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {[1, 2, 3, 4].map((step) => {
            const Icon = STEP_ICONS[step - 1];
            return (
              <div key={step} className="flex gap-3 items-start">
                <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary">
                  <Icon className="h-4 w-4" />
                </div>
                <div>
                  <div className="font-medium text-sm">
                    {step}. {t(`guide.step${step}.title`)}
                  </div>
                  <div className="text-sm text-muted-foreground mt-0.5">
                    {t(`guide.step${step}.desc`)}
                  </div>
                </div>
              </div>
            );
          })}
        </div>

        <DialogFooter className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Checkbox
              id="dont-show-again"
              checked={dontShowAgain}
              onCheckedChange={(v) => setDontShowAgain(!!v)}
            />
            <Label htmlFor="dont-show-again" className="text-sm text-muted-foreground">
              {t("common.doNotShowAgain")}
            </Label>
          </div>
          <Button onClick={handleClose}>
            {t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
