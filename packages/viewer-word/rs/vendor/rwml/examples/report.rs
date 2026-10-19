//! Authoring template: build a styled Korean operations report from data and
//! write a `.docx` with the public builder API.
//!
//! ```text
//! cargo run --example report -- report.docx
//! ```

use rwml::{
    Align, CellBuilder, Color, CommentBuilder, DocBuilder, ImageBuilder, PageNumberFormat,
    PageSetup, ParagraphBuilder, ParagraphStyleBuilder, RunBuilder, TableBuilder, TextDirection,
    VCell,
};

const KR_FONT: &str = "맑은 고딕";

fn run(text: &str) -> RunBuilder {
    RunBuilder::new(text).font(KR_FONT)
}

fn heading(level: u8, text: &str, color: Color) -> ParagraphBuilder {
    ParagraphBuilder::new()
        .heading_level(level)
        .spacing_before_pt(if level == 1 { 0.0 } else { 14.0 })
        .spacing_after_pt(8.0)
        .push_run(run(text).bold().color(color).build())
}

fn cell(text: &str, fill: Option<Color>, color: Option<Color>, bold: bool) -> CellBuilder {
    let mut builder = CellBuilder::new()
        .paragraph_runs([run(text)
            .color(color.unwrap_or_else(|| Color::rgb(0x22, 0x22, 0x22)))
            .build()])
        .valign(VCell::Center);
    if bold {
        builder = CellBuilder::new()
            .paragraph_runs([run(text)
                .bold()
                .color(color.unwrap_or_else(|| Color::rgb(0x22, 0x22, 0x22)))
                .build()])
            .valign(VCell::Center);
    }
    if let Some(fill) = fill {
        builder.shading(fill)
    } else {
        builder
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "report.docx".to_string());

    let navy = Color::rgb(0x1F, 0x38, 0x64);
    let red = Color::rgb(0xC0, 0x00, 0x00);
    let white = Color::rgb(0xFF, 0xFF, 0xFF);
    let zebra = Color::rgb(0xF2, 0xF2, 0xF2);
    let risk_fill = Color::rgb(0xFE, 0xF2, 0xF2);

    let header = |t: &str| cell(t, Some(navy), Some(white), true).header();
    let status_cell = |status: &str, urgent: bool, fill: Option<Color>| {
        let text = run(status)
            .color(if urgent {
                red
            } else {
                Color::rgb(0x22, 0x22, 0x22)
            })
            .bold()
            .build();
        let mut builder = CellBuilder::new()
            .paragraph_runs([text])
            .valign(VCell::Center);
        if let Some(fill) = fill {
            builder = builder.shading(fill);
        }
        builder
    };
    let due_cell = |due: &str, urgent: bool, fill: Option<Color>| {
        let mut text = run(due);
        if urgent {
            text = text.bold().color(red);
        }
        let mut builder = CellBuilder::new()
            .paragraph_runs([text.build()])
            .valign(VCell::Center);
        if let Some(fill) = fill {
            builder = builder.shading(fill);
        }
        builder
    };
    let task_row = |name: &str, team: &str, status: &str, due: &str, urgent: bool, fill| {
        [
            cell(name, fill, None, false),
            cell(team, fill, None, false),
            status_cell(status, urgent, fill),
            due_cell(due, urgent, fill),
        ]
    };

    let task_table = TableBuilder::new()
        .header_rows(1)
        .col_widths_pct([0.40, 0.22, 0.23, 0.15])
        .row([
            header("작업"),
            header("담당 부서"),
            header("상태"),
            header("완료 예정일"),
        ])
        .row(task_row(
            "문서 변환 점검",
            "플랫폼팀",
            "진행 중",
            "2026-06-25",
            true,
            None,
        ))
        .row(task_row(
            "템플릿 정리",
            "제품팀",
            "검토 중",
            "2026-07-10",
            false,
            Some(zebra),
        ))
        .row(task_row(
            "릴리스 노트 작성",
            "문서팀",
            "주의 필요",
            "2026-06-24",
            true,
            None,
        ));

    let mut builder = DocBuilder::new()
        .title("분기 운영 리포트")
        .creator("rwml")
        .document_id("6ECD4467")
        .web_extension_task_pane(
            "{52811C31-4593-43B8-A697-EB873422D156}",
            "af8fa5ba-4010-4bcc-9e03-a91ddadf6dd3",
            "1.0.0.0",
            "EXCatalog",
            "EXCatalog",
        )
        .margins_each_pt(54.0, 54.0, 54.0, 54.0)
        .title_page()
        .page_number_start(1)
        .page_number_format(PageNumberFormat::UpperRoman)
        .header_runs([run("분기 운영 리포트").bold().color(navy).build()])
        .first_header_runs([run("분기 운영 리포트 | 표지").bold().color(navy).build()])
        .even_header_runs([run("분기 운영 리포트 | 검토본").bold().color(navy).build()])
        .footer_runs([run("Page ").italic().build()])
        .first_footer_runs([run("대외 공유 전 내부 검토용").italic().build()])
        .even_footer_runs([run("짝수 쪽 검토 주석").italic().build()])
        .page_numbers()
        .paragraph_style(
            ParagraphStyleBuilder::new("ReportTitle", "Report Title")
                .based_on("Title")
                .align(Align::Center)
                .spacing_after_pt(18.0)
                .run_font(KR_FONT)
                .run_size_half_pt(32)
                .run_color(navy)
                .run_bold(),
        )
        .paragraph_style(
            ParagraphStyleBuilder::new("RiskCallout", "Risk Callout")
                .spacing_before_pt(6.0)
                .spacing_after_pt(8.0)
                .shading(risk_fill)
                .run_font(KR_FONT)
                .run_color(red)
                .run_bold(),
        )
        .rich_paragraph(
            ParagraphBuilder::new()
                .style("ReportTitle")
                .align(Align::Center)
                .push_run(run("분기 운영 리포트").bold().color(navy).build()),
        )
        .rich_paragraph(ParagraphBuilder::new().spacing_after_pt(10.0).runs([
            run("작성일 2026-06-22 - 아래 표에서 ").build(),
            run("주의 필요").bold().color(red).build(),
            run(" 항목을 빨간색으로 표시했습니다.").build(),
        ]))
        .rich_paragraph(
            ParagraphBuilder::new().style("RiskCallout").push_run(
                run("릴리스 노트 작성 일정 확인")
                    .comment(
                        CommentBuilder::new("담당자와 마감 시간을 재확인하세요.")
                            .author("Reviewer")
                            .initials("RV"),
                    )
                    .build(),
            ),
        )
        .rich_paragraph(heading(2, "작업 목록", navy))
        .rich_table(task_table)
        .section_break()
        .clear_header_footer()
        .page_setup(PageSetup {
            width_pt: 792.0,
            height_pt: 612.0,
            margin_pt: 54.0,
            landscape: true,
            ..PageSetup::default()
        })
        .page_number_start(1)
        .page_number_format(PageNumberFormat::Decimal)
        .header_runs([run("후속 조치").bold().color(navy).build()])
        .first_header_runs([run("후속 조치 | 시작 쪽").bold().color(navy).build()])
        .even_header_runs([run("후속 조치 | 짝수 쪽").bold().color(navy).build()])
        .footer_runs([
            run("후속 조치 - ").italic().build(),
            run("Page ").italic().build(),
        ])
        .first_footer_runs([run("후속 조치 시작").italic().build()])
        .even_footer_runs([run("후속 조치 짝수 쪽").italic().build()])
        .page_numbers()
        .rich_paragraph(heading(2, "후속 조치", navy))
        .numbered_list(["문서 변환 점검 결과 확인", "릴리스 노트 초안 검토"])
        .bullet_list_level(1, ["담당자 확인"])
        .field("FILENAME \\p", "report.docx")
        .hyperlink("프로젝트 링크", "https://example.com/");

    if let Ok(path) = std::env::var("RWML_IMAGE") {
        if let Ok(bytes) = std::fs::read(&path) {
            let mime = if path.ends_with(".jpg") || path.ends_with(".jpeg") {
                "image/jpeg"
            } else {
                "image/png"
            };
            builder = builder
                .rich_paragraph(heading(2, "처리 추이", navy))
                .rich_image(
                    ImageBuilder::new(bytes, mime)
                        .alt("처리 추이 차트")
                        .width_px(720),
                );
        }
    }

    builder = builder
        .section_break_even_page()
        .clear_header_footer()
        .page_setup(PageSetup {
            width_pt: 595.3,
            height_pt: 841.9,
            margin_pt: 60.0,
            ..PageSetup::default()
        })
        .columns(2)
        .doc_grid_lines_and_chars(360, 120)
        .text_direction(TextDirection::TopToBottomRightToLeft)
        .page_number_start(1)
        .page_number_format(PageNumberFormat::KoreanDigital)
        .header_runs([run("배포 품질 기준").bold().color(navy).build()])
        .first_header_runs([run("배포 품질 기준 | 첫 쪽").bold().color(navy).build()])
        .even_header_runs([run("배포 품질 기준 | 짝수 쪽").bold().color(navy).build()])
        .footer_runs([
            run("품질 기준 - ").italic().build(),
            run("Page ").italic().build(),
        ])
        .first_footer_runs([run("배포 기준 시작").italic().build()])
        .even_footer_runs([run("배포 기준 짝수 쪽").italic().build()])
        .page_numbers()
        .rich_paragraph(heading(2, "배포 및 품질 기준", navy))
        .rich_paragraph(ParagraphBuilder::new().spacing_after_pt(8.0).runs([
            run("인쇄 검토 부록은 두 단 구성과 문서 그리드를 사용해 ").build(),
            run("세로 배치").bold().color(navy).build(),
            run(" 승인 메모가 본문과 같은 패키지에서 유지되는지 확인합니다.").build(),
        ]))
        .numbered_list([
            "부서별 확인 의견은 같은 열 안에서 source-order 순서를 유지합니다.",
            "대외 배포 전 자동 작업 창 패키지가 문서와 함께 열리는지 확인합니다.",
        ])
        .bullet_list_level(1, ["문서 ID와 페이지 번호 정책은 배포본 추적 기준으로 사용합니다."])
        .section_break_odd_page()
        .clear_header_footer()
        .columns(1)
        .doc_grid_lines(360)
        .text_direction(TextDirection::LeftToRightTopToBottom)
        .page_number_start(1)
        .page_number_format(PageNumberFormat::DecimalZero)
        .header_runs([run("승인 메모").bold().color(navy).build()])
        .first_header_runs([run("승인 메모 | 첫 쪽").bold().color(navy).build()])
        .even_header_runs([run("승인 메모 | 짝수 쪽").bold().color(navy).build()])
        .footer_runs([
            run("승인 메모 - ").italic().build(),
            run("Page ").italic().build(),
        ])
        .first_footer_runs([run("최종 승인 시작").italic().build()])
        .even_footer_runs([run("최종 승인 짝수 쪽").italic().build()])
        .page_numbers()
        .rich_paragraph(heading(2, "승인 메모", navy))
        .rich_paragraph(ParagraphBuilder::new().spacing_after_pt(8.0).runs([
            run("운영, 제품, 문서 담당자는 위 일정과 품질 기준을 기준으로 최종 배포 여부를 판단합니다.")
                .build(),
        ]))
        .bullet_list(["승인 상태: 조건부 승인", "다음 검토: 2026-07-15"]);

    let model = builder.build();
    match std::fs::write(&out, rwml::write_docx(&model)) {
        Ok(()) => eprintln!("wrote {out}"),
        Err(e) => eprintln!("write {out}: {e}"),
    }
}
