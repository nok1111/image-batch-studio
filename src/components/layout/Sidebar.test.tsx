import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import i18n from "@/i18n";
import { Sidebar } from "./Sidebar";

describe("Sidebar", () => {
  it("renders all navigation entries in spec order", async () => {
    await i18n.changeLanguage("en");
    render(
      <MemoryRouter>
        <Sidebar />
      </MemoryRouter>,
    );
    const labels = screen.getAllByRole("link").map((a) => a.textContent);
    expect(labels).toEqual([
      "Dashboard",
      "Converter",
      "Compressor",
      "Resizer",
      "Watermark",
      "Bulk Renamer",
      "Metadata Cleaner",
      "Crop & Rotate",
      "Image Analyzer",
      "Settings",
    ]);
  });

  it("is translated", async () => {
    await i18n.changeLanguage("es");
    render(
      <MemoryRouter>
        <Sidebar />
      </MemoryRouter>,
    );
    expect(screen.getByRole("link", { name: "Ajustes" })).toBeInTheDocument();
  });
});
