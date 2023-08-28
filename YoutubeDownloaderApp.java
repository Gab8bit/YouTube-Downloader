import java.io.*;
import javax.swing.*;
import javax.swing.border.LineBorder;
import javax.swing.text.DefaultCaret;

import java.awt.*;
import java.awt.event.ActionEvent;
import java.awt.event.ActionListener;

public class YoutubeDownloaderApp extends JFrame {
    private JPanel mainPanel;
    private JPanel contentPanel;
    private JPanel finalContentPanel;
    private JPanel consolePanel;
    private JLabel urlLabel;
    private JTextField urlTextField;
    private JButton audioButton;
    private JButton videoButton;
    private JButton openFolderButton;
    private JButton resetButton;
    private JTextArea consoleTextArea;
    private int borderValue = 10;

    public YoutubeDownloaderApp() {
        setTitle("YouTube Downloader by Gab8bit");
        setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);
        setLayout(new BorderLayout());

        ImageIcon icon = new ImageIcon("./icons/logo.png");
        this.setIconImage(icon.getImage());

        mainPanel = new JPanel(new BorderLayout());

        contentPanel = new JPanel(new GridLayout(2, 2, 10, 10));
        finalContentPanel = new JPanel(new GridLayout(2, 2, 10, 10));
        consolePanel = new JPanel(new BorderLayout());

        urlLabel = new JLabel("URL");
        urlTextField = new JTextField(40);
        audioButton = new JButton("  Audio (MP3)", new ImageIcon("./icons/music.png"));
        videoButton = new JButton("  Video (MP4)", new ImageIcon("./icons/camera.png"));
        openFolderButton = new JButton(" Apri cartella", new ImageIcon("./icons/folder.png"));
        resetButton = new JButton(" Pulisci tutto", new ImageIcon("./icons/trash.png"));

        consoleTextArea = new JTextArea(30, 40);
        consoleTextArea.setEditable(false);
        JScrollPane scrollPane = new JScrollPane(consoleTextArea);
        DefaultCaret caret = (DefaultCaret)consoleTextArea.getCaret();
        caret.setUpdatePolicy(DefaultCaret.ALWAYS_UPDATE);

        contentPanel.setBorder(BorderFactory.createEmptyBorder(borderValue, borderValue, borderValue, borderValue));
        consolePanel.setBorder(BorderFactory.createEmptyBorder(borderValue, borderValue, borderValue, borderValue));
        urlTextField.setBorder(BorderFactory.createEmptyBorder(borderValue, borderValue, borderValue, borderValue));

        openFolderButton.setBorder(BorderFactory.createEmptyBorder(borderValue, borderValue, borderValue, borderValue));
        resetButton.setBorder(BorderFactory.createEmptyBorder(borderValue, borderValue, borderValue, borderValue));

        audioButton.setBackground(Color.CYAN);
        videoButton.setBackground(Color.YELLOW);
        openFolderButton.setBackground(Color.LIGHT_GRAY);
        resetButton.setBackground(Color.LIGHT_GRAY);

        urlLabel.setHorizontalAlignment(SwingConstants.RIGHT);

        audioButton.addActionListener(new ActionListener() {
            @Override
            public void actionPerformed(ActionEvent e) {
                if(urlTextField.getText().isEmpty()){
                    urlTextField.setBorder(new LineBorder(Color.RED));
                }else{
                    urlTextField.setBorder(null);
                    if(checkPlaylist(urlTextField.getText())){
                        showDownloadOptionsPopupMusic(urlTextField.getText());
                    }else executeDownloadCommand(".\\yt-dlp --update -x --no-playlist --audio-format mp3 --output \"downloaded\\%(title)s.%(ext)s\" \"" + urlTextField.getText() + "\"");
                }
            }
        });

        videoButton.addActionListener(new ActionListener() {
            @Override
            public void actionPerformed(ActionEvent e) {
                if(urlTextField.getText().isEmpty()){
                    urlTextField.setBorder(new LineBorder(Color.RED));
                }else{
                    if(checkPlaylist(urlTextField.getText())){
                        showDownloadOptionsPopupVideo(urlTextField.getText());
                    }else executeDownloadCommand(".\\yt-dlp --update --no-playlist -f mp4 --output \"downloaded\\%(title)s.%(ext)s\" \"" + urlTextField.getText() + "\"");
                }
            }
        });

        openFolderButton.addActionListener(new ActionListener() {
            @Override
            public void actionPerformed(ActionEvent e) {
                openFolder();
            }
        });

        resetButton.addActionListener(new ActionListener() {
            @Override
            public void actionPerformed(ActionEvent e) {
                urlTextField.setText("");
                consoleTextArea.setText("");
                urlTextField.setBorder(null);
            }
        });

        contentPanel.add(urlLabel);
        contentPanel.add(urlTextField);
        contentPanel.add(audioButton);
        contentPanel.add(videoButton);
        finalContentPanel.add(openFolderButton);
        finalContentPanel.add(resetButton);

        consolePanel.add(scrollPane, BorderLayout.CENTER);

        mainPanel.add(contentPanel, BorderLayout.NORTH);
        
        mainPanel.add(consolePanel, BorderLayout.CENTER);

        mainPanel.add(finalContentPanel, BorderLayout.SOUTH);
        //mainPanel.add(openFolderButton, BorderLayout.SOUTH);
        add(mainPanel, BorderLayout.CENTER);

        pack();
        setLocationRelativeTo(null);
    }

    private boolean checkPlaylist(String link){
        if(link.contains("list")){
            return true;
        }else return false;
    }

    private void openFolder() {
        try {
            Desktop.getDesktop().open(new File("./downloaded"));
        } catch (IOException e) {
            e.printStackTrace();
        }
    }

    private void showDownloadOptionsPopupMusic(final String link) {
        String[] options = {"Solo il video", "Tutta la playlist"};
        int choice = JOptionPane.showOptionDialog(this,
            "Questo video fa parte di una playlist. Vuoi scaricare solo il video o tutta la playlist?",
            "Opzioni di download",
            JOptionPane.YES_NO_OPTION,
            JOptionPane.QUESTION_MESSAGE,
            null,
            options,
            options[0]
        );

        if (choice == JOptionPane.YES_OPTION) {
            executeDownloadCommand(".\\yt-dlp --update -x --no-playlist --audio-format mp3 --output \"downloaded\\%(title)s.%(ext)s\" \"" + link + "\"");
        } else if (choice == JOptionPane.NO_OPTION) {
            executeDownloadCommand(".\\\\yt-dlp --update --ignore-errors --format bestaudio --extract-audio --audio-format mp3 --audio-quality 160K --output \"downloaded\\%(title)s.%(ext)s\" --yes-playlist \"" + link + "\"");
        }
    }

    private void showDownloadOptionsPopupVideo(final String link) {
        String[] options = {"Solo il video", "Tutta la playlist"};
        int choice = JOptionPane.showOptionDialog(this,
                "Questo video fa parte di una playlist. Vuoi scaricare solo il video o tutta la playlist?",
                "Opzioni di download",
                JOptionPane.YES_NO_OPTION,
                JOptionPane.QUESTION_MESSAGE,
                null,
                options,
                options[0]);

        if (choice == JOptionPane.YES_OPTION) {
            executeDownloadCommand(".\\yt-dlp --update --no-playlist -f mp4 --output \"downloaded\\%(title)s.%(ext)s\" \"" + link + "\"");
        } else if (choice == JOptionPane.NO_OPTION) {
            executeDownloadCommand(".\\yt-dlp --update --yes-playlist -f mp4 --output \"downloaded\\%(title)s.%(ext)s\"  \"" + link + "\""); 
        }
    }

    private void executeDownloadCommand(String command) {
        new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    consoleTextArea.setText("");
                    consoleTextArea.append("Controllo aggiornamenti e inizio download...\n");
                    ProcessBuilder processBuilder = new ProcessBuilder(command.split(" "));
                    processBuilder.redirectErrorStream(true);
                    Process process = processBuilder.start();
                    InputStream inputStream = process.getInputStream();
                    BufferedReader reader = new BufferedReader(new InputStreamReader(inputStream));
                    String line;
                    while ((line = reader.readLine()) != null) {
                        final String outputLine = line;
                        SwingUtilities.invokeLater(new Runnable() {
                            public void run() {
                                consoleTextArea.append(outputLine + "\n");
                            }
                        });
                    }
                    reader.close();
                    process.waitFor();
                } catch (IOException | InterruptedException e) {
                    e.printStackTrace();
                }
                SwingUtilities.invokeLater(new Runnable() {
                    public void run() {
                        consoleTextArea.append("\n------------------------------   DOWNLOAD COMPLETATO   ------------------------------\n");
                    }
                });
            }
        }).start();
    }

    public static void main(String[] args) {
        SwingUtilities.invokeLater(new Runnable() {
            @Override
            public void run() {
                YoutubeDownloaderApp app = new YoutubeDownloaderApp();
                app.setVisible(true);
            }
        });
    }
}

//https://www.youtube.com/watch?v=2AFMVvqJQjo&ab_channel=DAZNItalia
//https://www.youtube.com/watch?v=RzyD08-w-tk&list=PL4pduGYaRVkkOXVl44AVMoTaCXV8IU-lN