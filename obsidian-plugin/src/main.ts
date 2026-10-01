// Nectar Render pour Obsidian : un pont vers l'application installée.
//
// - « Ouvrir dans Nectar Render » lance l'atelier sur la note courante ;
// - « Exporter en PDF » produit le PDF à côté de la note, sans ouvrir l'atelier,
//   en appliquant les retouches déjà faites (dossier .nectar du coffre) ;
// - « Vérifier la mise en page » affiche ce que l'assistant a repéré.
//
// Tout reste local : le plugin ne fait qu'appeler les programmes installés.

import { spawn } from "child_process";
import { existsSync } from "fs";
import * as path from "path";
import {
  App,
  FileSystemAdapter,
  MarkdownView,
  Menu,
  Modal,
  Notice,
  Plugin,
  PluginSettingTab,
  Setting,
  TAbstractFile,
  TFile,
} from "obsidian";

interface NectarSettings {
  /** Chemin de l'atelier (nectar-render.exe). */
  appPath: string;
  /** Chemin de la ligne de commande (nectar.exe). */
  cliPath: string;
  /** Ouvrir le PDF après l'export. */
  openAfterExport: boolean;
}

/** Emplacement de l'installateur Windows (installation par utilisateur). */
function installDir(): string {
  const local = process.env.LOCALAPPDATA ?? path.join(process.env.USERPROFILE ?? "", "AppData", "Local");
  return path.join(local, "Programs", "Nectar Render");
}

const DEFAULTS: NectarSettings = {
  appPath: path.join(installDir(), "nectar-render.exe"),
  cliPath: path.join(installDir(), "nectar.exe"),
  openAfterExport: true,
};

export default class NectarRenderPlugin extends Plugin {
  settings: NectarSettings = { ...DEFAULTS };

  async onload() {
    this.settings = { ...DEFAULTS, ...(await this.loadData()) };

    this.addRibbonIcon("file-output", "Ouvrir dans Nectar Render", () => {
      const file = this.app.workspace.getActiveFile();
      if (file) this.openInWorkshop(file);
      else new Notice("Ouvre d'abord une note.");
    });

    this.addCommand({
      id: "ouvrir-atelier",
      name: "Ouvrir la note dans l'atelier Nectar Render",
      checkCallback: (checking) => this.withNote(checking, (file) => this.openInWorkshop(file)),
    });
    this.addCommand({
      id: "exporter-pdf",
      name: "Exporter la note en PDF (Nectar Render)",
      checkCallback: (checking) => this.withNote(checking, (file) => this.exportPdf(file)),
    });
    this.addCommand({
      id: "verifier",
      name: "Vérifier la mise en page (Nectar Render)",
      checkCallback: (checking) => this.withNote(checking, (file) => this.check(file)),
    });

    this.registerEvent(
      this.app.workspace.on("file-menu", (menu: Menu, file: TAbstractFile) => {
        if (!(file instanceof TFile) || file.extension !== "md") return;
        menu.addItem((item) =>
          item.setTitle("Ouvrir dans Nectar Render").setIcon("file-output").onClick(() => this.openInWorkshop(file)),
        );
        menu.addItem((item) =>
          item.setTitle("Exporter en PDF (Nectar)").setIcon("file-down").onClick(() => this.exportPdf(file)),
        );
      }),
    );

    this.addSettingTab(new NectarSettingTab(this.app, this));
  }

  async saveSettings() {
    await this.saveData(this.settings);
  }

  /** Exécute l'action sur la note Markdown active (palette de commandes). */
  private withNote(checking: boolean, action: (file: TFile) => void): boolean {
    const file = this.app.workspace.getActiveFile();
    if (!file || file.extension !== "md") return false;
    if (!checking) action(file);
    return true;
  }

  private fullPath(file: TFile): string | null {
    const adapter = this.app.vault.adapter;
    if (!(adapter instanceof FileSystemAdapter)) {
      new Notice("Nectar Render a besoin d'un coffre stocké sur le disque.");
      return null;
    }
    return path.join(adapter.getBasePath(), file.path);
  }

  private program(kind: "app" | "cli"): string | null {
    const program = kind === "app" ? this.settings.appPath : this.settings.cliPath;
    if (!existsSync(program)) {
      new Notice(`Nectar Render introuvable :\n${program}\nRègle le chemin dans les paramètres du plugin.`, 8000);
      return null;
    }
    return program;
  }

  /** Les modifications non enregistrées doivent être sur le disque avant l'export. */
  private async flush(file: TFile) {
    const view = this.app.workspace.getActiveViewOfType(MarkdownView);
    if (view && view.file?.path === file.path) await view.save();
  }

  async openInWorkshop(file: TFile) {
    const note = this.fullPath(file);
    const program = this.program("app");
    if (!note || !program) return;
    await this.flush(file);
    const child = spawn(program, [note], { detached: true, stdio: "ignore" });
    child.on("error", (e) => new Notice(`Lancement impossible : ${e.message}`));
    child.unref();
  }

  async exportPdf(file: TFile) {
    const note = this.fullPath(file);
    const program = this.program("cli");
    if (!note || !program) return;
    await this.flush(file);
    const output = note.replace(/\.md$/i, ".pdf");
    const notice = new Notice("Nectar Render : mise en page…", 0);
    const { code, stderr } = await run(program, ["export", note, "-o", output, "--system-fonts"]);
    notice.hide();
    if (code !== 0) {
      new Notice(`Export impossible :\n${stderr.trim().split("\n").slice(-4).join("\n")}`, 10000);
      return;
    }
    new Notice(`PDF exporté : ${path.basename(output)}`);
    if (this.settings.openAfterExport) {
      // Ouvre le PDF avec le lecteur par défaut du système.
      (require("electron") as any).shell.openPath(output);
    }
  }

  async check(file: TFile) {
    const note = this.fullPath(file);
    const program = this.program("cli");
    if (!note || !program) return;
    await this.flush(file);
    const { code, stdout, stderr } = await run(program, ["check", note]);
    new ReportModal(this.app, code === 0 ? stdout : stderr).open();
  }
}

function run(program: string, args: string[]): Promise<{ code: number; stdout: string; stderr: string }> {
  return new Promise((resolve) => {
    const child = spawn(program, args, { windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d.toString()));
    child.stderr.on("data", (d) => (stderr += d.toString()));
    child.on("error", (e) => resolve({ code: -1, stdout, stderr: e.message }));
    child.on("close", (code) => resolve({ code: code ?? -1, stdout, stderr }));
  });
}

class ReportModal extends Modal {
  constructor(
    app: App,
    private report: string,
  ) {
    super(app);
  }

  onOpen() {
    this.titleEl.setText("Nectar Render · vérification");
    const pre = this.contentEl.createEl("pre");
    pre.style.whiteSpace = "pre-wrap";
    pre.setText(this.report.trim() || "Rien à signaler.");
    const hint = this.contentEl.createEl("p");
    hint.setText("Ouvre la note dans l'atelier pour appliquer les corrections en un clic.");
  }

  onClose() {
    this.contentEl.empty();
  }
}

class NectarSettingTab extends PluginSettingTab {
  constructor(
    app: App,
    private plugin: NectarRenderPlugin,
  ) {
    super(app, plugin);
  }

  display() {
    const { containerEl } = this;
    containerEl.empty();
    new Setting(containerEl)
      .setName("Atelier (nectar-render.exe)")
      .setDesc("Chemin de l'application. L'installateur la place dans %LOCALAPPDATA%\\Programs\\Nectar Render.")
      .addText((text) =>
        text.setValue(this.plugin.settings.appPath).onChange(async (value) => {
          this.plugin.settings.appPath = value.trim();
          await this.plugin.saveSettings();
        }),
      );
    new Setting(containerEl)
      .setName("Ligne de commande (nectar.exe)")
      .setDesc("Utilisée pour l'export direct et la vérification.")
      .addText((text) =>
        text.setValue(this.plugin.settings.cliPath).onChange(async (value) => {
          this.plugin.settings.cliPath = value.trim();
          await this.plugin.saveSettings();
        }),
      );
    new Setting(containerEl)
      .setName("Ouvrir le PDF après l'export")
      .addToggle((toggle) =>
        toggle.setValue(this.plugin.settings.openAfterExport).onChange(async (value) => {
          this.plugin.settings.openAfterExport = value;
          await this.plugin.saveSettings();
        }),
      );
  }
}
